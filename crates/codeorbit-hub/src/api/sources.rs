//! 数据源管理端点

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

use codeorbit_contracts::{SourceDto, SourceStatusDto};

use super::app_state::AppState;
use crate::source_service;

/// 广播 source.statusChanged，携带最新源列表
async fn publish_sources(app: &AppState) {
    let data = serde_json::to_value(source_service::get_sources()).ok();
    app.state.read().await.publish("source.statusChanged", data);
}

async fn publish_source_operation(
    app: &AppState,
    result: &codeorbit_contracts::SourceOperationResultDto,
) {
    app.state
        .read()
        .await
        .publish("source.statusChanged", serde_json::to_value(result).ok());
}

/// GET /api/sources
pub async fn list_sources(State(_app): State<AppState>) -> Json<Vec<SourceDto>> {
    Json(source_service::get_sources())
}

/// GET /api/sources/:source 与 /api/sources/:source/status
pub async fn get_source_status(
    State(_app): State<AppState>,
    Path(source): Path<String>,
) -> Json<SourceStatusDto> {
    Json(source_service::get_source_status(&source))
}

/// POST /api/sources/:source/install
pub async fn install(State(app): State<AppState>, Path(source): Path<String>) -> impl IntoResponse {
    let result = source_service::install(&source);
    publish_source_operation(&app, &result).await;
    let status = if result.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result))
}

/// POST /api/sources/:source/uninstall
pub async fn uninstall(
    State(app): State<AppState>,
    Path(source): Path<String>,
) -> impl IntoResponse {
    let result = source_service::uninstall(&source);
    publish_source_operation(&app, &result).await;
    let status = if result.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result))
}

/// POST /api/sources/:source/repair
pub async fn repair(State(app): State<AppState>, Path(source): Path<String>) -> impl IntoResponse {
    let result = source_service::repair(&source);
    publish_source_operation(&app, &result).await;
    let status = if result.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result))
}

/// POST /api/sources/repair-all
pub async fn repair_all(State(app): State<AppState>) -> impl IntoResponse {
    let success = source_service::repair_all();
    publish_sources(&app).await;
    Json(json!({ "success": success }))
}
