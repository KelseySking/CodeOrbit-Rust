//! 端到端集成测试 — 完整流水线：Bridge(IPC) → HookServer → HubState → REST/WS
//!
//! 单测内串联所有场景（共享全局 OVERRIDE_ENV，避免并行竞争）。

use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;

use codeorbit_core::ipc::{
    IpcClient, OVERRIDE_ENV, full_path, read_message_async, write_message_async,
};
use codeorbit_hub::api::AppState;
use codeorbit_hub::{HookServer, HubState, router};

const TOKEN: &str = "e2e-token-abcdef1234567890";

fn event_json(event: &str, session: &str, tool: Option<&str>) -> String {
    let mut v = json!({
        "hook_event_name": event,
        "session_id": session,
        "_source": "claude"
    });
    if let Some(t) = tool {
        v["tool_name"] = json!(t);
    }
    v.to_string()
}

/// 模拟 Bridge：经 IPC 连接 Hub，发送 payload 并读取响应
async fn ipc_send(payload: &str, blocking: bool) -> Option<String> {
    let mut stream = IpcClient::connect(&full_path()).await.ok()?;
    write_message_async(&mut stream, payload).await.ok()?;
    if blocking {
        read_message_async(&mut stream).await.ok().flatten()
    } else {
        tokio::time::timeout(Duration::from_secs(3), read_message_async(&mut stream))
            .await
            .ok()?
            .ok()
            .flatten()
    }
}

async fn next_json<S>(ws: &mut S) -> Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("ws timeout")
            .expect("ws closed")
            .expect("ws error");
        if let Message::Text(t) = msg {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

async fn rest(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {TOKEN}"));
    let req = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn full_pipeline_e2e() {
    // ---- 启动完整服务栈 ----
    let pipe = format!("codeorbit-e2e-{}", std::process::id());
    // SAFETY: 测试内一次性设置进程环境变量
    unsafe {
        std::env::set_var(OVERRIDE_ENV, &pipe);
    }

    let state = Arc::new(RwLock::new(HubState::new()));
    tokio::spawn(HookServer::run(state.clone(), Duration::from_secs(5)));

    let app = router(AppState::new(state.clone(), TOKEN, true));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    {
        let served = app.clone();
        tokio::spawn(async move {
            axum::serve(listener, served).await.unwrap();
        });
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    // ---- WebSocket 连接 ----
    let url = format!("ws://{addr}/api/events?token={TOKEN}");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // ---- 非阻塞事件：SessionStart 经 IPC → ack {} ----
    let ack = ipc_send(&event_json("SessionStart", "e2e", None), false).await;
    assert_eq!(ack.as_deref(), Some("{}"), "非阻塞事件应返回 {{}} ack");

    // WS 收到 session.updated
    let ev = next_json(&mut ws).await;
    assert_eq!(ev["type"], "session.updated");
    assert_eq!(ev["data"][0]["sessionId"], "e2e");

    // REST 能查到该会话
    let (status, body) = rest(&app, "GET", "/api/sessions", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["sessionId"], "e2e");

    // ---- 阻塞事件：PermissionRequest 经 IPC（等待用户决策）----
    let ipc_task = tokio::spawn(async move {
        ipc_send(&event_json("PermissionRequest", "e2e", Some("Bash")), true).await
    });

    // 轮询 pending 直到出现
    let action_id = loop {
        let (_, pending) = rest(&app, "GET", "/api/pending", None).await;
        if let Some(first) = pending.as_array().and_then(|a| a.first()) {
            break first["actionId"].as_str().unwrap().to_string();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    // REST 批准
    let (status, _) = rest(
        &app,
        "POST",
        &format!("/api/permissions/{action_id}/allow"),
        Some(json!({ "always": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Bridge 侧 IPC 响应应解析为 allow
    let response = ipc_task.await.unwrap().expect("阻塞事件应有响应");
    assert!(response.contains("allow"), "响应应含 allow: {response}");

    // pending 已清空
    let (_, pending) = rest(&app, "GET", "/api/pending", None).await;
    assert_eq!(pending.as_array().unwrap().len(), 0);

    ws.close(None).await.ok();
    unsafe {
        std::env::remove_var(OVERRIDE_ENV);
    }
}
