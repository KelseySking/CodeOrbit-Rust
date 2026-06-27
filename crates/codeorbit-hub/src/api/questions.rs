//! 问题决策端点

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use codeorbit_contracts::{
    QuestionAnswerRequest, QuestionCurrentAnswerRequest, QuestionCurrentAnswerResultDto,
};

use super::app_state::AppState;
use super::error::AppError;
use super::parse_body_or_default;

/// POST /api/questions/:id/answer
pub async fn answer(
    State(app): State<AppState>,
    Path(action_id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    let request: QuestionAnswerRequest = parse_body_or_default(&body);
    let ok = app.state.write().await.answer_question(&action_id, request);
    if ok {
        Ok(Json(json!({ "success": true })))
    } else {
        Err(AppError::NotFound("Pending action not found"))
    }
}

/// POST /api/questions/:id/answer-current
pub async fn answer_current(
    State(app): State<AppState>,
    Path(action_id): Path<String>,
    body: Bytes,
) -> Result<Json<QuestionCurrentAnswerResultDto>, AppError> {
    let request: QuestionCurrentAnswerRequest = parse_body_or_default(&body);
    let (found, resolved) =
        app.state
            .write()
            .await
            .answer_current_question(&action_id, request.answers, request.actor);
    if found {
        Ok(Json(QuestionCurrentAnswerResultDto {
            success: true,
            resolved,
        }))
    } else {
        Err(AppError::NotFound("Pending action not found"))
    }
}

/// POST /api/questions/:id/dismiss
pub async fn dismiss(
    State(app): State<AppState>,
    Path(action_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let ok = app
        .state
        .write()
        .await
        .dismiss_question(&action_id, "dismissed", None);
    if ok {
        Ok(Json(json!({ "success": true })))
    } else {
        Err(AppError::NotFound("Pending action not found"))
    }
}
