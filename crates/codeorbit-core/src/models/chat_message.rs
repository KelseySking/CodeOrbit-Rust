use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub is_user: bool,
    pub text: String,
    pub timestamp: DateTime<Utc>,
}

impl ChatMessage {
    pub fn new(is_user: bool, text: impl Into<String>) -> Self {
        Self {
            is_user,
            text: text.into(),
            timestamp: Utc::now(),
        }
    }
}
