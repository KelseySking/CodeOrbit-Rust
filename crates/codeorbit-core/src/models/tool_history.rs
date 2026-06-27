use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 工具调用历史条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHistoryEntry {
    pub tool_name: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub success: bool,
}
