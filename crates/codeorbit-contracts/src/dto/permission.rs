use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 权限请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestDto {
    pub session_id: String,
    pub tool_name: String,
    pub tool_use_id: Option<String>,
    pub tool_input: Option<HashMap<String, serde_json::Value>>,
    pub description: Option<String>,
    pub hook_event_name: String,
}

/// 权限决策请求 (客户端 → 服务端)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDecisionRequest {
    #[serde(default)]
    pub always: bool,
    pub reason: Option<String>,
    pub actor: Option<String>,
}
