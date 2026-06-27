//! 待处理操作端点

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use codeorbit_contracts::{PendingActionDto, PendingHistoryDto};

use super::app_state::AppState;
use super::error::AppError;

/// GET /api/pending
pub async fn list_pending(State(app): State<AppState>) -> Json<Vec<PendingActionDto>> {
    Json(app.state.read().await.get_pending_actions())
}

/// GET /api/pending/:id
pub async fn get_pending(
    State(app): State<AppState>,
    Path(action_id): Path<String>,
) -> Result<Json<PendingActionDto>, AppError> {
    app.state
        .read()
        .await
        .get_pending_action(&action_id)
        .map(Json)
        .ok_or(AppError::NotFound("Resource not found"))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
}

/// GET /api/pending/history
pub async fn pending_history(
    State(app): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Json<PendingHistoryDto> {
    let limit = query.limit.unwrap_or(100);
    Json(PendingHistoryDto {
        entries: app.state.read().await.get_pending_history(limit),
    })
}
