//! IPC 客户端 — 连接 Hub 的 Named Pipe / Unix Socket，发送富化 payload 并读取响应
//!
//! 关键原则：Bridge 是观察者，任何失败都静默返回 None（调用方据此 exit 0，不阻断宿主 CLI）。

use std::time::Duration;

use codeorbit_core::ipc::{IpcClient, full_path, read_message_async, write_message_async};

const BLOCKING_CONNECT_TIMEOUT: Duration = Duration::from_millis(3000);
const NON_BLOCKING_CONNECT_TIMEOUT: Duration = Duration::from_millis(1500);
const NON_BLOCKING_READ_TIMEOUT: Duration = Duration::from_secs(3);

/// 发送富化 payload 到 Hub 并读取响应。
///
/// 返回 `Some(response)` 表示成功（响应可能是 `{}` ack）；`None` 表示连接/读写失败（静默）。
pub async fn send(enriched_json: &str, blocking: bool) -> Option<String> {
    let connect_timeout = if blocking {
        BLOCKING_CONNECT_TIMEOUT
    } else {
        NON_BLOCKING_CONNECT_TIMEOUT
    };

    // 连接（带超时）
    let mut stream = tokio::time::timeout(connect_timeout, IpcClient::connect(&full_path()))
        .await
        .ok()?
        .ok()?;

    // 写入消息（带超时）
    tokio::time::timeout(
        connect_timeout,
        write_message_async(&mut stream, enriched_json),
    )
    .await
    .ok()?
    .ok()?;

    // 读取响应
    let response = if blocking {
        // 阻塞事件：不加客户端读超时（由宿主 CLI / Hub AppState 控制，最长可达 24h）
        read_message_async(&mut stream).await.ok()?
    } else {
        // 非阻塞事件：3 秒读超时等待 Hub 的 ack
        tokio::time::timeout(NON_BLOCKING_READ_TIMEOUT, read_message_async(&mut stream))
            .await
            .ok()?
            .ok()?
    };

    Some(response.unwrap_or_else(|| "{}".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeorbit_core::ipc::{IpcServer, OVERRIDE_ENV};

    // 两个场景合并为单个测试：避免并行修改同一全局环境变量 OVERRIDE_ENV
    #[tokio::test]
    async fn ipc_client_send_behavior() {
        let pid = std::process::id();

        // 1. 无服务端 → 连接失败 → None
        // SAFETY: 测试内设置进程环境变量
        unsafe {
            std::env::set_var(OVERRIDE_ENV, format!("codeorbit-bridge-absent-{pid}"));
        }
        assert_eq!(send("{}", false).await, None, "无 Hub 时应返回 None");

        // 2. 有服务端 → 完整往返
        unsafe {
            std::env::set_var(OVERRIDE_ENV, format!("codeorbit-bridge-test-{pid}"));
        }
        let mut server = IpcServer::bind(&full_path()).await.unwrap();
        let server_task = tokio::spawn(async move {
            let mut stream = server.accept().await.unwrap();
            let received = read_message_async(&mut stream).await.unwrap();
            assert_eq!(received.as_deref(), Some("{\"hook_event_name\":\"Stop\"}"));
            write_message_async(&mut stream, "{\"ok\":true}")
                .await
                .unwrap();
        });

        let response = send("{\"hook_event_name\":\"Stop\"}", false).await;
        assert_eq!(response.as_deref(), Some("{\"ok\":true}"));

        server_task.await.unwrap();
        unsafe {
            std::env::remove_var(OVERRIDE_ENV);
        }
    }
}
