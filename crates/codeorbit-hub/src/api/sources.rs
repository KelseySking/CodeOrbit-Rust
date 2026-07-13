//! 数据源管理端点

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;

use codeorbit_contracts::{SourceDto, SourceOperationResultDto, SourceStatusDto};
use codeorbit_core::services::log_error;

use super::app_state::AppState;
use crate::source_service;

fn log_source_failure(op: &str, result: &SourceOperationResultDto, distro: Option<&str>) {
    if result.success {
        return;
    }
    let mut fields = vec![
        ("op", op),
        ("source", result.source.as_str()),
        ("status", "400"),
        ("message", result.message.as_str()),
    ];
    if let Some(d) = distro {
        fields.push(("distro", d));
    }
    log_error("api.sources", &result.message, &fields);
}

#[derive(Default, Deserialize)]
pub struct WslQuery {
    distro: Option<String>,
}

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

/// GET /api/sources/wsl/distros
pub async fn list_wsl_distros(State(_app): State<AppState>) -> impl IntoResponse {
    match source_service::list_wsl_distros() {
        Ok(distros) => (StatusCode::OK, Json(json!({ "distros": distros }))),
        Err(message) => {
            log_error(
                "api.sources",
                &message,
                &[("op", "list_wsl_distros"), ("status", "400")],
            );
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "distros": [], "message": message })),
            )
        }
    }
}

/// GET /api/sources/:source/wsl/status?distro=Ubuntu
pub async fn get_wsl_source_status(
    State(_app): State<AppState>,
    Path(source): Path<String>,
    Query(query): Query<WslQuery>,
) -> Json<SourceStatusDto> {
    Json(source_service::get_wsl_source_status(
        &source,
        query.distro.as_deref(),
    ))
}

/// POST /api/sources/:source/install
pub async fn install(State(app): State<AppState>, Path(source): Path<String>) -> impl IntoResponse {
    let result = source_service::install(&source);
    log_source_failure("install", &result, None);
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
    log_source_failure("uninstall", &result, None);
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
    log_source_failure("repair", &result, None);
    publish_source_operation(&app, &result).await;
    let status = if result.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result))
}

/// POST /api/sources/:source/wsl/install?distro=Ubuntu
pub async fn install_wsl(
    State(app): State<AppState>,
    Path(source): Path<String>,
    Query(query): Query<WslQuery>,
) -> impl IntoResponse {
    let result = source_service::install_wsl(&source, query.distro.as_deref());
    log_source_failure("wsl_install", &result, query.distro.as_deref());
    publish_source_operation(&app, &result).await;
    let status = if result.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result))
}

/// POST /api/sources/:source/wsl/uninstall?distro=Ubuntu
pub async fn uninstall_wsl(
    State(app): State<AppState>,
    Path(source): Path<String>,
    Query(query): Query<WslQuery>,
) -> impl IntoResponse {
    let result = source_service::uninstall_wsl(&source, query.distro.as_deref());
    log_source_failure("wsl_uninstall", &result, query.distro.as_deref());
    publish_source_operation(&app, &result).await;
    let status = if result.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result))
}

/// POST /api/sources/:source/wsl/repair?distro=Ubuntu
pub async fn repair_wsl(
    State(app): State<AppState>,
    Path(source): Path<String>,
    Query(query): Query<WslQuery>,
) -> impl IntoResponse {
    let result = source_service::repair_wsl(&source, query.distro.as_deref());
    log_source_failure("wsl_repair", &result, query.distro.as_deref());
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
    if !success {
        log_error(
            "api.sources",
            "repair_all failed",
            &[("op", "repair_all"), ("status", "200")],
        );
    }
    publish_sources(&app).await;
    Json(json!({ "success": success }))
}
