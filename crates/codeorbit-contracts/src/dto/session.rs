use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageDto {
    pub is_user: bool,
    pub text: String,
    pub timestamp_utc: DateTime<Utc>,
}

/// 工具历史条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolHistoryEntryDto {
    pub tool_name: String,
    pub timestamp_utc: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub success: bool,
}

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub session_id: String,
    pub source: String,
    pub source_display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_tool_description: Option<String>,
    pub created_at_utc: DateTime<Utc>,
    pub last_updated_at_utc: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracked_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracked_process_started_at_utc: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_user_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_assistant_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    pub transcript_position: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_session_id: Option<String>,
    pub recent_messages: Vec<ChatMessageDto>,
    pub tool_history: Vec<ToolHistoryEntryDto>,
}
