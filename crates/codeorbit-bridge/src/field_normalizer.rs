//! 字段名标准化 — 将不同 CLI 的命名风格统一为标准 snake_case 名

use serde_json::{Map, Value};

/// 标准化字段名：补充标准名（保留原始字段，仅在标准名缺失时添加）
pub fn normalize_field_names(payload: &mut Map<String, Value>) {
    // 事件名：hookEventName / eventName / event → hook_event_name（首个命中者胜）
    for alias in ["hookEventName", "eventName", "event"] {
        if !payload.contains_key("hook_event_name")
            && let Some(value) = payload.get(alias).cloned()
        {
            payload.insert("hook_event_name".to_string(), value);
        }
    }

    // 会话 ID：sessionId → session_id
    if !payload.contains_key("session_id")
        && let Some(value) = payload.get("sessionId").cloned()
    {
        payload.insert("session_id".to_string(), value);
    }

    // 工具名：toolName → tool_name（Copilot 等）
    if !payload.contains_key("tool_name")
        && let Some(value) = payload.get("toolName").cloned()
    {
        payload.insert("tool_name".to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_camel_case_aliases() {
        let mut payload: Map<String, Value> = json!({
            "hookEventName": "PreToolUse",
            "sessionId": "s1",
            "toolName": "Bash"
        })
        .as_object()
        .unwrap()
        .clone();

        normalize_field_names(&mut payload);

        assert_eq!(payload["hook_event_name"], "PreToolUse");
        assert_eq!(payload["session_id"], "s1");
        assert_eq!(payload["tool_name"], "Bash");
        // 原始字段保留
        assert_eq!(payload["hookEventName"], "PreToolUse");
    }

    #[test]
    fn does_not_override_existing_standard_name() {
        let mut payload: Map<String, Value> = json!({
            "hook_event_name": "Stop",
            "event": "PreToolUse"
        })
        .as_object()
        .unwrap()
        .clone();

        normalize_field_names(&mut payload);
        assert_eq!(payload["hook_event_name"], "Stop");
    }
}
