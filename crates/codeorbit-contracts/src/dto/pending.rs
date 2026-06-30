use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::permission::PermissionRequestDto;
use super::question::QuestionDto;

/// 待处理操作
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingActionDto {
    pub action_id: String,
    pub kind: String,
    pub session_id: String,
    pub source: String,
    pub source_display_name: String,
    pub project_name: Option<String>,
    pub working_directory: Option<String>,
    pub created_at_utc: DateTime<Utc>,
    pub permission: Option<PermissionRequestDto>,
    pub question: Option<QuestionDto>,
}

/// 待处理操作解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingResolutionDto {
    pub action_id: String,
    pub kind: String,
    pub session_id: Option<String>,
    pub source: Option<String>,
    pub decision: String,
    pub actor: Option<String>,
    pub reason: Option<String>,
    pub resolved_at_utc: DateTime<Utc>,
}

/// 待处理操作历史
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingHistoryDto {
    pub entries: Vec<PendingResolutionDto>,
}
