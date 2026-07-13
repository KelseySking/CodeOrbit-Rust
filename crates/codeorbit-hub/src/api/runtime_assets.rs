//! 运行时资源端点

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use codeorbit_contracts::RuntimeAssetsDto;
use codeorbit_core::services::log_error;

use super::app_state::AppState;
use crate::source_service;

/// GET /api/runtime-assets
pub async fn get_runtime_assets(State(_app): State<AppState>) -> Json<RuntimeAssetsDto> {
    Json(source_service::get_runtime_assets())
}

/// POST /api/runtime-assets/repair
pub async fn repair(State(app): State<AppState>) -> Json<Value> {
    let success = source_service::repair_runtime_assets();
    if !success {
        log_error(
            "api.runtime_assets",
            "repair_runtime_assets failed",
            &[("op", "repair")],
        );
    }
    let assets = source_service::get_runtime_assets();

    // 广播源状态变化
    let data = serde_json::to_value(source_service::get_sources()).ok();
    app.state.read().await.publish("source.statusChanged", data);

    Json(json!({ "success": success, "assets": assets }))
}
