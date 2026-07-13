//! Hook 服务端 — 监听 IPC，接收 Bridge 的富化 payload 并分派到 HubState
//!
//! 设计：单 accept 循环 + 每连接 spawn 处理任务（功能上等效并发；core 的 `IpcServer::accept`
//! 取 `&mut self`，不支持多个并发 accept）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::RwLock;

use codeorbit_core::ipc::{
    IpcServer, IpcStream, full_path, read_message_async, write_message_async,
};
use codeorbit_core::models::HookEvent;
use codeorbit_core::services::{hook_tool_classifier, log_error, normalize_event_name};

use crate::state::{HubState, handle_blocking_event};

const READ_MESSAGE_TIMEOUT: Duration = Duration::from_secs(10);

/// Hook 服务端
pub struct HookServer;

impl HookServer {
    /// 持续监听并处理 Bridge 连接（由调用方 spawn，并在关闭时 abort）
    pub async fn run(state: Arc<RwLock<HubState>>, session_timeout: Duration) {
        let path = full_path();
        let mut server = match IpcServer::bind(&path).await {
            Ok(s) => s,
            Err(e) => {
                let detail = e.to_string();
                tracing::error!("HookServer 绑定失败: {detail}");
                log_error(
                    "hook_server",
                    &detail,
                    &[("op", "bind"), ("path", path.as_str())],
                );
                return;
            }
        };

        loop {
            match server.accept().await {
                Ok(stream) => {
                    let state = state.clone();
                    tokio::spawn(handle_connection(stream, state, session_timeout));
                }
                Err(e) => {
                    let detail = e.to_string();
                    tracing::warn!("HookServer accept 错误: {detail}");
                    log_error("hook_server", &detail, &[("op", "accept")]);
                }
            }
        }
    }
}

async fn handle_connection(
    mut stream: IpcStream,
    state: Arc<RwLock<HubState>>,
    session_timeout: Duration,
) {
    // 读取消息（10s 超时）
    let json =
        match tokio::time::timeout(READ_MESSAGE_TIMEOUT, read_message_async(&mut stream)).await {
            Ok(Ok(Some(j))) if !j.trim().is_empty() => j,
            Ok(Ok(Some(_))) => {
                log_error(
                    "hook_server",
                    "empty hook payload",
                    &[("op", "read_message")],
                );
                let _ = write_message_async(&mut stream, "{}").await;
                return;
            }
            Ok(Ok(None)) => {
                log_error(
                    "hook_server",
                    "client closed before message",
                    &[("op", "read_message")],
                );
                let _ = write_message_async(&mut stream, "{}").await;
                return;
            }
            Ok(Err(e)) => {
                let detail = e.to_string();
                log_error(
                    "hook_server",
                    &detail,
                    &[("op", "read_message")],
                );
                let _ = write_message_async(&mut stream, "{}").await;
                return;
            }
            Err(_) => {
                log_error(
                    "hook_server",
                    "read message timeout",
                    &[("op", "read_message"), ("timeout_secs", "10")],
                );
                let _ = write_message_async(&mut stream, "{}").await;
                return;
            }
        };

    let Some(evt) = serde_json::from_str::<Value>(&json)
        .ok()
        .and_then(|v| HookEvent::from_json(&v, None))
    else {
        let preview: String = json.chars().take(200).collect();
        log_error(
            "hook_server",
            "invalid hook payload",
            &[("op", "parse"), ("preview", preview.as_str())],
        );
        let _ = write_message_async(&mut stream, "{}").await;
        return;
    };

    if is_blocking(&evt) {
        let response = handle_blocking_event(&state, evt, session_timeout).await;
        let _ = write_message_async(&mut stream, &response).await;
    } else {
        // 非阻塞：先 ack，再 fire-and-forget 更新状态
        let _ = write_message_async(&mut stream, "{}").await;
        state.write().await.handle_event(&evt);
    }
}

fn is_blocking(evt: &HookEvent) -> bool {
    let source = evt.source.as_deref().unwrap_or("unknown");
    let name = normalize_event_name(source, &evt.event_name);

    if name == "PermissionRequest" {
        return true;
    }
    if name == "PreToolUse" && hook_tool_classifier::should_block_question_tool(evt, &name) {
        return true;
    }
    if (name == "Notification" || name.starts_with("Question"))
        && (contains_any(Some(&evt.raw_json), &["question", "questions"])
            || contains_any(evt.tool_input.as_ref(), &["question", "questions"]))
    {
        return true;
    }
    name == "PreToolUse"
        && (has_approval_signal(Some(&evt.raw_json))
            || has_approval_signal(evt.tool_input.as_ref()))
}

fn contains_any(element: Option<&Value>, names: &[&str]) -> bool {
    let Some(Value::Object(obj)) = element else {
        return false;
    };
    for (key, value) in obj {
        if names.iter().any(|n| n.eq_ignore_ascii_case(key)) {
            return true;
        }
        if value.is_object() && contains_any(Some(value), names) {
            return true;
        }
    }
    false
}

fn has_approval_signal(element: Option<&Value>) -> bool {
    let Some(Value::Object(obj)) = element else {
        return false;
    };
    for (key, value) in obj {
        if is_approval_key(key) && is_truthy(value) {
            return true;
        }
        if value.is_object() && has_approval_signal(Some(value)) {
            return true;
        }
    }
    false
}

fn is_approval_key(key: &str) -> bool {
    matches!(
        key,
        "permission_request"
            | "permissionRequest"
            | "requires_approval"
            | "requiresApproval"
            | "approval_required"
            | "approvalRequired"
    )
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::String(s) => !s.eq_ignore_ascii_case("false") && s != "0" && !s.trim().is_empty(),
        Value::Number(n) => n.as_i64().map(|i| i != 0).unwrap_or(true),
        Value::Null => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeorbit_core::ipc::{IpcClient, OVERRIDE_ENV};
    use serde_json::json;

    #[tokio::test]
    async fn dispatches_non_blocking_event_and_acks() {
        let pipe = format!("codeorbit-hookserver-test-{}", std::process::id());
        // SAFETY: 测试内设置进程环境变量
        unsafe {
            std::env::set_var(OVERRIDE_ENV, &pipe);
        }

        let state = Arc::new(RwLock::new(HubState::new()));
        let server = tokio::spawn(HookServer::run(state.clone(), Duration::from_secs(5)));

        // 给服务端一点时间绑定
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 模拟 Bridge：连接并发送 SessionStart
        let mut client = IpcClient::connect(&full_path()).await.unwrap();
        let evt = json!({
            "hook_event_name": "SessionStart",
            "session_id": "host-test",
            "_source": "claude"
        })
        .to_string();
        write_message_async(&mut client, &evt).await.unwrap();
        let ack = read_message_async(&mut client).await.unwrap();
        assert_eq!(ack.as_deref(), Some("{}"));

        // 等待 fire-and-forget 落地
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(state.read().await.get_session("host-test").is_some());

        server.abort();
        unsafe {
            std::env::remove_var(OVERRIDE_ENV);
        }
    }
}
