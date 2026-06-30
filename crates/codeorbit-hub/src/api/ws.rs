//! WebSocket 实时推送 — GET /api/events，订阅 HubState 广播并转发给客户端
//!
//! 设计：HubState 已用 `tokio::sync::broadcast` 扇出事件，每个连接订阅各自的
//! receiver，天然支持多客户端、非阻塞发布与 lagged 处理，无需手动维护客户端表。

use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use tokio::sync::broadcast::error::RecvError;

use codeorbit_contracts::HubEventDto;

use super::app_state::AppState;

/// 单客户端发送超时
const SEND_TIMEOUT: Duration = Duration::from_secs(5);

/// GET /api/events — WebSocket 升级入口
pub async fn events_handler(State(app): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

async fn handle_socket(mut socket: WebSocket, app: AppState) {
    let mut rx = app.state.read().await.subscribe();

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    // 客户端关闭或连接结束
                    Some(Ok(Message::Close(_))) | None => break,
                    // 其它入站消息（ping 由 axum 自动回应）忽略
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            event = rx.recv() => {
                match event {
                    Ok(evt) => {
                        // 5 秒发送超时：超时或失败则关闭该连接
                        let sent = tokio::time::timeout(SEND_TIMEOUT, send_event(&mut socket, &evt))
                            .await
                            .map(|r| r.is_ok())
                            .unwrap_or(false);
                        if !sent {
                            break;
                        }
                    }
                    // 慢客户端：丢失旧事件但不断开
                    Err(RecvError::Lagged(dropped)) => {
                        tracing::warn!("realtime client lagged, dropped {dropped} events");
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }

    let _ = socket.send(Message::Close(None)).await;
}

async fn send_event(socket: &mut WebSocket, event: &HubEventDto) -> Result<(), axum::Error> {
    let text = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
    socket.send(Message::Text(text.into())).await
}
