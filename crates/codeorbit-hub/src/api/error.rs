//! API 错误类型 — 统一 ApiErrorDto 响应 + error.log 落盘

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use codeorbit_contracts::ApiErrorDto;
use codeorbit_core::services::log_error;

/// API 错误
#[derive(Debug)]
pub enum AppError {
    NotFound(&'static str),
    Unauthorized(&'static str),
    BadRequest(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, &str, String) {
        match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.to_string()),
            AppError::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, "unauthorized", msg.to_string())
            }
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.parts();
        let status_s = status.as_u16().to_string();
        log_error(
            "api",
            &message,
            &[
                ("code", code),
                ("status", status_s.as_str()),
            ],
        );
        (
            status,
            Json(ApiErrorDto {
                code: code.to_string(),
                message,
            }),
        )
            .into_response()
    }
}
