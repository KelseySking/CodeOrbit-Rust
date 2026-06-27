//! 事件名与字段名标准化

use crate::sources::adapter_registry;

/// 标准化事件名为内部 PascalCase 名称
pub fn normalize_event_name(source: &str, raw_event_name: &str) -> String {
    let raw_name = raw_event_name.trim();

    // 优先尝试源特定的事件别名映射
    let registry = adapter_registry::global();
    if let Some(source_specific) = registry
        .get(Some(source))
        .try_normalize_event_name(raw_name)
    {
        return source_specific;
    }

    // 通用 snake_case / camelCase / PascalCase / lowercase 映射
    let mapped = match raw_name.to_lowercase().as_str() {
        "permission_request" | "permissionrequest" => "PermissionRequest",
        "permission_denied" | "permissiondenied" => "PermissionDenied",
        "pre_tool_use" | "pretooluse" => "PreToolUse",
        "post_tool_use" | "posttooluse" => "PostToolUse",
        "post_tool_use_failure" | "posttoolusefailure" => "PostToolUseFailure",
        "user_prompt_submit" | "userpromptsubmit" => "UserPromptSubmit",
        "session_start" | "sessionstart" => "SessionStart",
        "session_end" | "sessionend" => "SessionEnd",
        "subagent_start" | "subagentstart" => "SubagentStart",
        "subagent_stop" | "subagentstop" => "SubagentStop",
        "pre_compact" | "precompact" => "PreCompact",
        "post_compact" | "postcompact" => "PostCompact",
        "stop" => "Stop",
        "notification" => "Notification",
        // 未识别事件按原始大小写透传
        _ => return raw_name.to_string(),
    };
    mapped.to_string()
}

/// 标准化字段名
pub fn normalize_field_name(raw_field_name: &str) -> String {
    match raw_field_name {
        "hook_event_name" | "hookEventName" | "event_name" | "eventName" => "event_name",
        "session_id" | "sessionId" => "session_id",
        "tool_name" | "toolName" | "tool" => "tool_name",
        "tool_use_id" | "toolUseId" => "tool_use_id",
        "tool_input" | "toolInput" | "input" | "arguments" | "args" => "tool_input",
        other => return other.to_string(),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_snake_and_camel_case() {
        assert_eq!(
            normalize_event_name("unknown", "pre_tool_use"),
            "PreToolUse"
        );
        assert_eq!(normalize_event_name("unknown", "preToolUse"), "PreToolUse");
        assert_eq!(
            normalize_event_name("unknown", "PermissionRequest"),
            "PermissionRequest"
        );
        assert_eq!(normalize_event_name("unknown", "stop"), "Stop");
    }

    #[test]
    fn passes_through_unknown_events() {
        assert_eq!(
            normalize_event_name("unknown", "CustomEvent"),
            "CustomEvent"
        );
    }

    #[test]
    fn normalizes_field_names() {
        assert_eq!(normalize_field_name("hookEventName"), "event_name");
        assert_eq!(normalize_field_name("toolName"), "tool_name");
        assert_eq!(normalize_field_name("args"), "tool_input");
        assert_eq!(normalize_field_name("unknown_field"), "unknown_field");
    }
}
