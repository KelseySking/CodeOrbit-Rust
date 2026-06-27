//! Hook 工具分类 — 从事件载荷递归提取工具名，判定问题类工具

use serde_json::Value;

use crate::models::HookEvent;

const MAX_DEPTH: u32 = 8;

const TOOL_NAME_KEYS: &[&str] = &[
    "tool_name",
    "toolName",
    "tool",
    "name",
    "function_name",
    "functionName",
];

const NESTED_OBJECT_KEYS: &[&str] = &[
    "tool",
    "function",
    "payload",
    "data",
    "request",
    "env",
    "environment",
];

/// 问题类工具种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookQuestionToolKind {
    None,
    AskUserQuestion,
    CodexRequestUserInput,
}

/// 获取事件的工具名
pub fn get_tool_name(evt: &HookEvent) -> Option<String> {
    first_non_blank(evt.tool_name.as_deref()).or_else(|| get_tool_name_from_value(&evt.raw_json, 0))
}

/// 从事件判定问题工具种类
pub fn get_question_tool_kind_from_event(evt: &HookEvent) -> HookQuestionToolKind {
    get_question_tool_kind(get_tool_name(evt).as_deref())
}

/// 从工具名判定问题工具种类
pub fn get_question_tool_kind(tool_name: Option<&str>) -> HookQuestionToolKind {
    let Some(name) = tool_name else {
        return HookQuestionToolKind::None;
    };
    if name.eq_ignore_ascii_case("AskUserQuestion") {
        return HookQuestionToolKind::AskUserQuestion;
    }
    if name.eq_ignore_ascii_case("request_user_input")
        || name.eq_ignore_ascii_case("functions.request_user_input")
    {
        return HookQuestionToolKind::CodexRequestUserInput;
    }
    HookQuestionToolKind::None
}

pub fn is_ask_user_question(evt: &HookEvent) -> bool {
    get_question_tool_kind_from_event(evt) == HookQuestionToolKind::AskUserQuestion
}

pub fn is_codex_request_user_input(evt: &HookEvent) -> bool {
    get_question_tool_kind_from_event(evt) == HookQuestionToolKind::CodexRequestUserInput
}

pub fn is_question_tool(evt: &HookEvent) -> bool {
    get_question_tool_kind_from_event(evt) != HookQuestionToolKind::None
}

/// 是否应阻塞该问题工具（CodexRequestUserInput 仅在 PreToolUse 阻塞）
pub fn should_block_question_tool(evt: &HookEvent, normalized_event_name: &str) -> bool {
    let kind = get_question_tool_kind_from_event(evt);
    match kind {
        HookQuestionToolKind::None => false,
        HookQuestionToolKind::CodexRequestUserInput => {
            normalized_event_name.eq_ignore_ascii_case("PreToolUse")
        }
        HookQuestionToolKind::AskUserQuestion => true,
    }
}

fn get_tool_name_from_value(value: &Value, depth: u32) -> Option<String> {
    if depth > MAX_DEPTH {
        return None;
    }

    match value {
        Value::String(s) => first_non_blank(Some(s)),
        Value::Array(items) => items
            .iter()
            .find_map(|item| get_tool_name_from_value(item, depth + 1)),
        Value::Object(_) => {
            for key in TOOL_NAME_KEYS {
                if let Some(prop) = get_property_ignore_case(value, key) {
                    match prop {
                        Value::String(s) => {
                            if let Some(found) = first_non_blank(Some(s)) {
                                return Some(found);
                            }
                        }
                        Value::Object(_) | Value::Array(_) => {
                            if let Some(found) = get_tool_name_from_value(prop, depth + 1) {
                                return Some(found);
                            }
                        }
                        _ => {}
                    }
                }
            }
            for key in NESTED_OBJECT_KEYS {
                if let Some(prop @ (Value::Object(_) | Value::Array(_))) =
                    get_property_ignore_case(value, key)
                    && let Some(found) = get_tool_name_from_value(prop, depth + 1)
                {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn get_property_ignore_case<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let obj = value.as_object()?;
    obj.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}

fn first_non_blank(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event_with_raw(raw: Value) -> HookEvent {
        HookEvent::from_json(&raw, None).unwrap_or(HookEvent {
            event_name: "PreToolUse".into(),
            session_id: None,
            tool_name: None,
            tool_use_id: None,
            agent_id: None,
            tool_input: None,
            raw_json: raw,
            source: None,
            parent_pid: None,
            tracked_pid: None,
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        })
    }

    #[test]
    fn extracts_nested_tool_name() {
        let evt = event_with_raw(json!({
            "hook_event_name": "PreToolUse",
            "payload": { "function": { "name": "AskUserQuestion" } }
        }));
        assert_eq!(get_tool_name(&evt).as_deref(), Some("AskUserQuestion"));
        assert!(is_ask_user_question(&evt));
    }

    #[test]
    fn classifies_codex_request_user_input() {
        assert_eq!(
            get_question_tool_kind(Some("functions.request_user_input")),
            HookQuestionToolKind::CodexRequestUserInput
        );
    }

    #[test]
    fn block_rules() {
        let evt = event_with_raw(json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "request_user_input"
        }));
        assert!(should_block_question_tool(&evt, "PreToolUse"));
        assert!(!should_block_question_tool(&evt, "PermissionRequest"));
    }
}
