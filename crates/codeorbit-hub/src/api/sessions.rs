//! 会话管理端点

use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use codeorbit_contracts::{ChatMessageDto, SessionDto};

use super::app_state::AppState;
use super::error::AppError;

/// GET /api/sessions
pub async fn list_sessions(State(app): State<AppState>) -> Json<Vec<SessionDto>> {
    Json(app.state.read().await.get_sessions())
}

/// GET /api/sessions/:id
pub async fn get_session(
    State(app): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionDto>, AppError> {
    app.state
        .read()
        .await
        .get_session(&session_id)
        .map(Json)
        .ok_or(AppError::NotFound("Session not found"))
}

/// GET /api/sessions/:id/messages
pub async fn get_session_messages(
    State(app): State<AppState>,
    Path(session_id): Path<String>,
) -> Json<Vec<ChatMessageDto>> {
    Json(app.state.read().await.get_session_messages(&session_id))
}

/// POST /api/sessions/:id/dismiss
pub async fn dismiss_session(
    State(app): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    if app.state.write().await.dismiss_session(&session_id) {
        Ok(Json(json!({ "success": true })))
    } else {
        Err(AppError::NotFound("Session not found"))
    }
}

/// POST /api/sessions/:id/activate-terminal
///
/// 会话存在时广播 `terminal.activate` 事件（携带终端信息），由 HUD 客户端自行聚焦终端。
pub async fn activate_terminal(
    State(app): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let guard = app.state.read().await;
    let Some(session) = guard.get_session(&session_id) else {
        return Err(AppError::NotFound("Session not found"));
    };
    guard.publish(
        "terminal.activate",
        Some(json!({
            "sessionId": session.session_id,
            "terminalApp": session.terminal_app,
            "terminalSessionId": session.terminal_session_id,
            "pid": session.tracked_pid,
        })),
    );
    Ok(Json(json!({ "success": true })))
}
