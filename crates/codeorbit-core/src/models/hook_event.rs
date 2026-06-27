use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::models::SupportedSource;

/// 从 AI 工具 Hook 接收的事件
///
/// 使用 `serde_json::Value` 进行灵活解析，支持多种字段名变体。
/// 这是 Rust 的优势：类型安全 + 动态 JSON 解析的结合。
#[derive(Debug, Clone)]
pub struct HookEvent {
    pub event_name: String,
    pub session_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_use_id: Option<String>,
    pub agent_id: Option<String>,
    pub tool_input: Option<Value>,
    pub raw_json: Value,

    // Bridge 注入的元数据
    pub source: Option<String>,
    pub parent_pid: Option<u32>,
    pub tracked_pid: Option<u32>,
    pub tracked_pid_kind: Option<String>,
    pub tracked_process_started_at_utc: Option<DateTime<Utc>>,
}

impl HookEvent {
    /// 从 JSON 解析 HookEvent，接受多种字段名变体
    pub fn from_json(json: &Value, source: Option<&str>) -> Option<Self> {
        let event_name = get_string_field(
            json,
            &[
                "hook_event_name",
                "hookEventName",
                "event_name",
                "eventName",
                "event",
            ],
        )?;

        if event_name.is_empty() {
            return None;
        }

        let normalized_source = source.and_then(Self::normalize_source).or_else(|| {
            get_string_field(
                json,
                &[
                    "_source",
                    "source",
                    "CodeOrbit_SOURCE",
                    "CodeOrbit_source",
                    "tool_source",
                    "toolSource",
                ],
            )
            .and_then(|s| Self::normalize_source(&s))
        });

        Some(Self {
            event_name,
            session_id: get_string_field(json, &["session_id", "sessionId"]),
            tool_name: get_string_field(json, &["tool_name", "toolName", "tool", "name"]),
            tool_use_id: get_string_field(json, &["tool_use_id", "toolUseId"]),
            agent_id: get_string_field(json, &["agent_id", "agentId"]),
            tool_input: get_nested_field(
                json,
                &[
                    "tool_input",
                    "toolInput",
                    "input",
                    "arguments",
                    "args",
                    "params",
                ],
            ),
            raw_json: json.clone(),
            source: normalized_source,
            parent_pid: get_int_field(json, &["_ppid", "_hook_ppid"]),
            tracked_pid: get_int_field(json, &["_tracked_pid"]),
            tracked_pid_kind: get_string_field(json, &["_tracked_pid_kind"]),
            tracked_process_started_at_utc: get_datetime_field(
                json,
                &["_tracked_process_started_at_utc"],
            ),
        })
    }

    fn normalize_source(source: &str) -> Option<String> {
        let normalized = source.trim();
        if normalized.is_empty() {
            return None;
        }
        if SupportedSource::is_valid(normalized) {
            Some(normalized.to_string())
        } else {
            None
        }
    }
}

/// 从 JSON 提取字符串字段，支持多种键名
fn get_string_field(json: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = json.get(*key)
            && let Some(s) = value.as_str()
        {
            return Some(s.to_string());
        }
    }

    // 尝试嵌套查找
    for nest_key in &["tool", "payload", "data", "env", "environment"] {
        if let Some(nested) = json.get(*nest_key)
            && nested.is_object()
            && let Some(result) = get_string_field(nested, keys)
        {
            return Some(result);
        }
    }

    None
}

/// 从 JSON 提取整数字段
fn get_int_field(json: &Value, keys: &[&str]) -> Option<u32> {
    for key in keys {
        if let Some(value) = json.get(*key)
            && let Some(n) = value.as_u64()
        {
            return Some(n as u32);
        }
    }
    None
}

/// 从 JSON 提取嵌套对象字段
fn get_nested_field(json: &Value, keys: &[&str]) -> Option<Value> {
    for key in keys {
        if let Some(value) = json.get(*key)
            && !value.is_null()
        {
            return Some(value.clone());
        }
    }
    None
}

/// 从 JSON 提取日期时间字段
fn get_datetime_field(json: &Value, keys: &[&str]) -> Option<DateTime<Utc>> {
    for key in keys {
        if let Some(value) = json.get(*key)
            && let Some(s) = value.as_str()
            && let Ok(dt) = s.parse::<DateTime<Utc>>()
        {
            return Some(dt);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_snake_case() {
        let json = json!({
            "hook_event_name": "PreToolUse",
            "session_id": "abc123",
            "tool_name": "Bash"
        });
        let event = HookEvent::from_json(&json, None).unwrap();
        assert_eq!(event.event_name, "PreToolUse");
        assert_eq!(event.session_id, Some("abc123".to_string()));
        assert_eq!(event.tool_name, Some("Bash".to_string()));
    }

    #[test]
    fn parse_camel_case() {
        let json = json!({
            "hookEventName": "PostToolUse",
            "sessionId": "xyz789",
            "toolName": "Read"
        });
        let event = HookEvent::from_json(&json, None).unwrap();
        assert_eq!(event.event_name, "PostToolUse");
        assert_eq!(event.session_id, Some("xyz789".to_string()));
    }

    #[test]
    fn parse_with_source() {
        let json = json!({
            "hook_event_name": "SessionStart",
            "_source": "claude",
            "_tracked_pid": 12345
        });
        let event = HookEvent::from_json(&json, Some("codex")).unwrap();
        assert_eq!(event.source, Some("codex".to_string()));
        assert_eq!(event.tracked_pid, Some(12345));
    }
}
