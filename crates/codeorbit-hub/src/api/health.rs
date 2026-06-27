//! 健康与元数据端点

use axum::Json;
use axum::extract::State;

use codeorbit_contracts::{ApiCapabilitiesDto, ApiHealthDto, ApiVersionDto};

use super::app_state::AppState;

/// GET /api/health（免认证）
pub async fn health(State(app): State<AppState>) -> Json<ApiHealthDto> {
    Json(ApiHealthDto {
        status: "ok".to_string(),
        started_at_utc: app.started_at,
    })
}

/// GET /api/version
pub async fn version() -> Json<ApiVersionDto> {
    Json(ApiVersionDto {
        product: "CodeOrbit Runtime".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /api/capabilities
pub async fn capabilities(State(app): State<AppState>) -> Json<ApiCapabilitiesDto> {
    Json(ApiCapabilitiesDto {
        hook_injection: true,
        approval: true,
        question: true,
        transcript: true,
        realtime: true,
        realtime_protocols: vec!["websocket".to_string()],
        security_mode: if app.loopback {
            "localhost-token".to_string()
        } else {
            "remote-token".to_string()
        },
    })
}
