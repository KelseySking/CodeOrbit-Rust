//! 权限决策端点

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use codeorbit_contracts::PermissionDecisionRequest;

use super::app_state::AppState;
use super::error::AppError;
use super::parse_body_or_default;

/// POST /api/permissions/:id/allow
pub async fn allow(
    State(app): State<AppState>,
    Path(action_id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    let request: PermissionDecisionRequest = parse_body_or_default(&body);
    let ok = app
        .state
        .write()
        .await
        .allow_permission(&action_id, request.always, request.actor);
    pending_result(ok)
}

/// POST /api/permissions/:id/deny
pub async fn deny(
    State(app): State<AppState>,
    Path(action_id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    let request: PermissionDecisionRequest = parse_body_or_default(&body);
    let reason = request.reason.unwrap_or_else(|| "user denied".to_string());
    let ok = app
        .state
        .write()
        .await
        .deny_permission(&action_id, &reason, request.actor);
    pending_result(ok)
}

fn pending_result(ok: bool) -> Result<Json<Value>, AppError> {
    if ok {
        Ok(Json(json!({ "success": true })))
    } else {
        Err(AppError::NotFound("Pending action not found"))
    }
}
