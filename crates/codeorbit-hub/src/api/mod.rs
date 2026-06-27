//! REST API 层 — 路由、认证、端点处理器

pub mod app_state;
pub mod auth_middleware;
pub mod error;
pub mod health;
pub mod pending;
pub mod permissions;
pub mod questions;
pub mod runtime_assets;
pub mod sessions;
pub mod sources;
pub mod token_store;
pub mod ws;

use axum::Router;
use axum::body::Bytes;
use axum::middleware;
use axum::routing::{get, post};

pub use app_state::AppState;
pub use token_store::ensure_token;

/// 解析请求体；空 body 或解析失败时返回默认值（对齐 C# ReadBodyAsync）
pub(crate) fn parse_body_or_default<T: serde::de::DeserializeOwned + Default>(body: &Bytes) -> T {
    if body.is_empty() {
        return T::default();
    }
    serde_json::from_slice(body).unwrap_or_default()
}

/// 构建 `/api` 路由树，并挂载认证中间件
pub fn router(state: AppState) -> Router {
    Router::new()
        // 健康与元数据
        .route("/api/health", get(health::health))
        .route("/api/version", get(health::version))
        .route("/api/capabilities", get(health::capabilities))
        // 会话
        .route("/api/sessions", get(sessions::list_sessions))
        .route("/api/sessions/{session_id}", get(sessions::get_session))
        .route(
            "/api/sessions/{session_id}/messages",
            get(sessions::get_session_messages),
        )
        .route(
            "/api/sessions/{session_id}/dismiss",
            post(sessions::dismiss_session),
        )
        .route(
            "/api/sessions/{session_id}/activate-terminal",
            post(sessions::activate_terminal),
        )
        // 待处理操作（静态 history 路由需先于动态 :id 注册）
        .route("/api/pending", get(pending::list_pending))
        .route("/api/pending/history", get(pending::pending_history))
        .route("/api/pending/{action_id}", get(pending::get_pending))
        // 权限决策
        .route(
            "/api/permissions/{action_id}/allow",
            post(permissions::allow),
        )
        .route("/api/permissions/{action_id}/deny", post(permissions::deny))
        // 问题决策
        .route("/api/questions/{action_id}/answer", post(questions::answer))
        .route(
            "/api/questions/{action_id}/answer-current",
            post(questions::answer_current),
        )
        .route(
            "/api/questions/{action_id}/dismiss",
            post(questions::dismiss),
        )
        // WebSocket 实时推送
        .route("/api/events", get(ws::events_handler))
        // 数据源管理
        .route("/api/sources", get(sources::list_sources))
        .route("/api/sources/repair-all", post(sources::repair_all))
        .route("/api/sources/{source}", get(sources::get_source_status))
        .route(
            "/api/sources/{source}/status",
            get(sources::get_source_status),
        )
        .route("/api/sources/{source}/install", post(sources::install))
        .route("/api/sources/{source}/uninstall", post(sources::uninstall))
        .route("/api/sources/{source}/repair", post(sources::repair))
        // 运行时资源
        .route(
            "/api/runtime-assets",
            get(runtime_assets::get_runtime_assets),
        )
        .route("/api/runtime-assets/repair", post(runtime_assets::repair))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware::authorize,
        ))
        .with_state(state)
}
