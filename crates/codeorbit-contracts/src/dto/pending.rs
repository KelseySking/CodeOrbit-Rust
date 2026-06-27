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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    pub created_at_utc: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<PermissionRequestDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<QuestionDto>,
}

/// 待处理操作解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingResolutionDto {
    pub action_id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub resolved_at_utc: DateTime<Utc>,
}

/// 待处理操作历史
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingHistoryDto {
    pub entries: Vec<PendingResolutionDto>,
}
