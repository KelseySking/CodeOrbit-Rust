//! 事件分类 — 判定 payload 是否为需等待 Hub 响应的阻塞事件

use serde_json::{Map, Value};

use codeorbit_core::models::HookEvent;
use codeorbit_core::services::{hook_tool_classifier, normalize_event_name};

/// 判定是否为阻塞事件
pub fn is_blocking_event(payload: &Map<String, Value>) -> bool {
    let event_name = get_string_value(
        payload,
        &[
            "hook_event_name",
            "event_name",
            "event",
            "hookEventName",
            "eventName",
        ],
    )
    .unwrap_or_default();
    let source =
        get_string_value(payload, &["_source", "source"]).unwrap_or_else(|| "unknown".to_string());
    let normalized = normalize_event_name(&source, &event_name);

    if normalized == "PermissionRequest" {
        return true;
    }
    if normalized == "PreToolUse" && should_block_question_tool(payload, &normalized, &source) {
        return true;
    }
    if normalized == "PreToolUse" && has_approval_needed_signal(payload) {
        return true;
    }
    if (normalized == "Notification" || normalized.to_lowercase().contains("question"))
        && has_question_payload(payload)
    {
        return true;
    }
    false
}

fn should_block_question_tool(
    payload: &Map<String, Value>,
    normalized: &str,
    source: &str,
) -> bool {
    let value = Value::Object(payload.clone());
    match HookEvent::from_json(&value, Some(source)) {
        Some(evt) => hook_tool_classifier::should_block_question_tool(&evt, normalized),
        None => false,
    }
}

fn has_question_payload(payload: &Map<String, Value>) -> bool {
    payload_contains_question(payload.get("tool_input"))
        || payload_contains_question(payload.get("toolInput"))
        || payload_contains_question(payload.get("input"))
        || payload.contains_key("question")
        || payload.contains_key("questions")
}

fn payload_contains_question(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Object(obj)) => obj.contains_key("question") || obj.contains_key("questions"),
        Some(Value::String(s)) => s.to_lowercase().contains("question"),
        _ => false,
    }
}

fn has_approval_needed_signal(payload: &Map<String, Value>) -> bool {
    payload_contains_approval(payload.get("tool_input"))
        || payload_contains_approval(payload.get("toolInput"))
        || payload_contains_approval(payload.get("input"))
        || payload
            .iter()
            .any(|(k, v)| is_approval_signal_name(k) && is_truthy_signal(v))
}

fn payload_contains_approval(value: Option<&Value>) -> bool {
    let Some(Value::Object(obj)) = value else {
        return false;
    };
    for (key, val) in obj {
        if is_approval_signal_name(key) && is_truthy_signal(val) {
            return true;
        }
        if val.is_object() && payload_contains_approval(Some(val)) {
            return true;
        }
    }
    false
}

fn is_approval_signal_name(name: &str) -> bool {
    [
        "permission_request",
        "permissionRequest",
        "requires_approval",
        "requiresApproval",
        "approval_required",
        "approvalRequired",
    ]
    .iter()
    .any(|n| n.eq_ignore_ascii_case(name))
}

fn is_truthy_signal(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::String(s) => !s.eq_ignore_ascii_case("false") && s != "0",
        Value::Object(_) => true,
        _ => false,
    }
}

fn get_string_value(payload: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        match payload.get(*key) {
            Some(Value::Null) | None => continue,
            Some(Value::String(s)) => return Some(s.clone()),
            Some(other) => return Some(other.to_string()),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn permission_request_blocks() {
        assert!(is_blocking_event(&map(json!({
            "hook_event_name": "PermissionRequest",
            "_source": "claude"
        }))));
    }

    #[test]
    fn pretooluse_with_approval_signal_blocks() {
        assert!(is_blocking_event(&map(json!({
            "hook_event_name": "PreToolUse",
            "_source": "claude",
            "tool_input": { "requires_approval": true }
        }))));
        // 顶层信号
        assert!(is_blocking_event(&map(json!({
            "hook_event_name": "PreToolUse",
            "_source": "claude",
            "approval_required": true
        }))));
    }

    #[test]
    fn pretooluse_with_question_tool_blocks() {
        assert!(is_blocking_event(&map(json!({
            "hook_event_name": "PreToolUse",
            "_source": "claude",
            "tool_name": "AskUserQuestion"
        }))));
    }

    #[test]
    fn notification_with_question_blocks() {
        assert!(is_blocking_event(&map(json!({
            "hook_event_name": "Notification",
            "_source": "claude",
            "question": "Proceed?"
        }))));
    }

    #[test]
    fn plain_events_do_not_block() {
        assert!(!is_blocking_event(&map(json!({
            "hook_event_name": "PreToolUse",
            "_source": "claude",
            "tool_name": "Bash"
        }))));
        assert!(!is_blocking_event(&map(json!({
            "hook_event_name": "Stop",
            "_source": "claude"
        }))));
        // false 信号不阻塞
        assert!(!is_blocking_event(&map(json!({
            "hook_event_name": "PreToolUse",
            "_source": "claude",
            "requires_approval": false
        }))));
    }
}
