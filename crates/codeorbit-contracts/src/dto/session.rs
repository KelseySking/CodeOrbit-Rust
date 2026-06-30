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
    pub project_name: Option<String>,
    pub working_directory: Option<String>,
    pub status: String,
    pub current_tool_name: Option<String>,
    pub current_tool_description: Option<String>,
    pub created_at_utc: DateTime<Utc>,
    pub last_updated_at_utc: DateTime<Utc>,
    pub tracked_pid: Option<u32>,
    pub tracked_process_started_at_utc: Option<DateTime<Utc>>,
    pub last_user_prompt: Option<String>,
    pub last_assistant_message: Option<String>,
    pub completion_text: Option<String>,
    pub transcript_path: Option<String>,
    pub transcript_position: i64,
    pub terminal_app: Option<String>,
    pub terminal_session_id: Option<String>,
    pub recent_messages: Vec<ChatMessageDto>,
    pub tool_history: Vec<ToolHistoryEntryDto>,
}
