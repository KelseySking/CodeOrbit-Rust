//! WebSocket 实时推送集成测试 — 真实 server + WS 客户端

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;

use codeorbit_core::models::HookEvent;
use codeorbit_hub::api::AppState;
use codeorbit_hub::{HubState, router};

const TOKEN: &str = "ws-test-token-1234567890abcdef";

async fn spawn_server() -> (std::net::SocketAddr, Arc<RwLock<HubState>>) {
    let state = Arc::new(RwLock::new(HubState::new()));
    let app = router(AppState::new(state.clone(), TOKEN, true));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, state)
}

fn session_start(session_id: &str) -> HookEvent {
    HookEvent {
        event_name: "SessionStart".to_string(),
        session_id: Some(session_id.to_string()),
        tool_name: None,
        tool_use_id: None,
        agent_id: None,
        tool_input: None,
        raw_json: json!({ "hook_event_name": "SessionStart" }),
        source: Some("claude".to_string()),
        parent_pid: None,
        tracked_pid: None,
        tracked_pid_kind: None,
        tracked_process_started_at_utc: None,
    }
}

/// 读取下一条文本帧并解析为 JSON（带超时）
async fn next_json<S>(ws: &mut S) -> Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("timed out waiting for message")
            .expect("stream ended")
            .expect("ws error");
        if let Message::Text(text) = msg {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

#[tokio::test]
async fn ws_broadcasts_events_without_welcome_frame() {
    let (addr, state) = spawn_server().await;

    let url = format!("ws://{addr}/api/events?token={TOKEN}");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 触发状态变更
    state.write().await.handle_event(&session_start("s1"));

    // 应收到 session.updated 广播
    let event = next_json(&mut ws).await;
    assert_eq!(event["type"], "session.updated");
    assert!(event["data"].is_array());
    assert_eq!(event["data"][0]["sessionId"], "s1");

    ws.close(None).await.ok();
}

#[tokio::test]
async fn ws_multi_client_broadcast() {
    let (addr, state) = spawn_server().await;
    let url = format!("ws://{addr}/api/events?token={TOKEN}");

    let (mut a, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let (mut b, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    state.write().await.handle_event(&session_start("multi"));

    // 两个客户端都应收到
    assert_eq!(next_json(&mut a).await["type"], "session.updated");
    assert_eq!(next_json(&mut b).await["type"], "session.updated");

    a.close(None).await.ok();
    b.close(None).await.ok();
}

#[tokio::test]
async fn ws_requires_token() {
    let (addr, _state) = spawn_server().await;
    let url = format!("ws://{addr}/api/events");
    // 无 token 的升级请求应被认证中间件拒绝（非 101）
    let result = tokio_tungstenite::connect_async(&url).await;
    assert!(result.is_err(), "无 token 的 WS 连接应失败");
}
