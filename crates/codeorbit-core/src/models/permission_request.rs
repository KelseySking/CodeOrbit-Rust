use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 权限请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub session_id: String,
    pub tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub hook_event_name: String,
}

impl PermissionRequest {
    /// 是否为安全的内部工具（可自动审批）
    pub fn is_safe_internal_tool(&self) -> bool {
        matches!(self.tool_name.as_str(), "Read" | "Grep" | "Glob" | "LS")
    }
}

impl Default for PermissionRequest {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            tool_name: String::new(),
            tool_use_id: None,
            tool_input: None,
            description: None,
            hook_event_name: "PermissionRequest".to_string(),
        }
    }
}
