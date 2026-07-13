//! 认证中间件 — Bearer / X-CodeOrbit-Token / query 三来源校验，/api/health 豁免

use axum::Json;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use codeorbit_contracts::ApiErrorDto;
use codeorbit_core::services::log_error;

use super::app_state::AppState;

/// 认证中间件
pub async fn authorize(State(app): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if path.eq_ignore_ascii_case("/api/health") {
        return next.run(request).await;
    }

    if is_authorized(&app, &request) {
        return next.run(request).await;
    }

    let method = request.method().as_str().to_string();
    log_error(
        "api",
        "Missing or invalid CodeOrbit API token",
        &[
            ("code", "unauthorized"),
            ("status", "401"),
            ("method", method.as_str()),
            ("path", path),
        ],
    );

    (
        StatusCode::UNAUTHORIZED,
        Json(ApiErrorDto {
            code: "unauthorized".to_string(),
            message: "Missing or invalid CodeOrbit API token".to_string(),
        }),
    )
        .into_response()
}

fn is_authorized(app: &AppState, request: &Request) -> bool {
    let token = app.token.as_str();
    if token.is_empty() {
        return false;
    }

    let headers = request.headers();

    if let Some(auth) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok())
        && auth.len() > 7
        && auth[..7].eq_ignore_ascii_case("Bearer ")
        && &auth[7..] == token
    {
        return true;
    }

    if let Some(value) = headers
        .get("X-CodeOrbit-Token")
        .and_then(|v| v.to_str().ok())
        && value == token
    {
        return true;
    }

    if let Some(query) = request.uri().query() {
        for pair in query.split('&') {
            if let Some(value) = pair.strip_prefix("token=")
                && value == token
            {
                return true;
            }
        }
    }

    false
}
