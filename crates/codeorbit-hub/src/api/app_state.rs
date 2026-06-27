//! API 应用状态 — 在所有路由处理器间共享

use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::state::HubState;

/// Axum 共享状态
#[derive(Clone)]
pub struct AppState {
    pub state: Arc<RwLock<HubState>>,
    pub token: Arc<String>,
    pub started_at: DateTime<Utc>,
    /// 是否绑定在回环地址（决定 capabilities 的 security_mode）
    pub loopback: bool,
}

impl AppState {
    pub fn new(state: Arc<RwLock<HubState>>, token: impl Into<String>, loopback: bool) -> Self {
        Self {
            state,
            token: Arc::new(token.into()),
            started_at: Utc::now(),
            loopback,
        }
    }
}
