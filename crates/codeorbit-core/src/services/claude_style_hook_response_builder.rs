//! Claude 风格 Hook 响应格式构建器

use serde_json::{Map, Value, json};

use super::event_normalizer::normalize_event_name;
use super::legacy_question_response_builder::{Answers, join_answers};
use crate::models::{HookEvent, QuestionData};

fn source_of(evt: &HookEvent) -> &str {
    evt.source.as_deref().unwrap_or("unknown")
}

pub(crate) fn build_permission_allow_response(evt: &HookEvent, always: bool) -> String {
    let normalized = normalize_event_name(source_of(evt), &evt.event_name);
    if normalized == "PreToolUse" {
        return json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "permissionDecisionReason": if always {
                    "User chose always allow"
                } else {
                    "User allowed this operation"
                },
            }
        })
        .to_string();
    }

    json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": { "behavior": "allow" },
        }
    })
    .to_string()
}

pub(crate) fn build_permission_deny_response(evt: &HookEvent, reason: &str) -> String {
    let normalized = normalize_event_name(source_of(evt), &evt.event_name);
    if normalized == "PreToolUse" {
        return json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        })
        .to_string();
    }

    json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": { "behavior": "deny", "reason": reason },
        }
    })
    .to_string()
}

pub(crate) fn build_ask_user_question_answer_response(
    evt: &HookEvent,
    question: &QuestionData,
    answers: &Answers,
) -> String {
    let hook_name = normalize_event_name(source_of(evt), &evt.event_name);

    let mut updated_input = copy_original_question_input(question);
    let mut answer_object = Map::new();
    for (key, values) in answers {
        answer_object.insert(key.clone(), Value::String(join_answers(values)));
    }
    updated_input.insert("answers".to_string(), Value::Object(answer_object));

    let mut hook_specific = Map::new();
    hook_specific.insert(
        "hookEventName".to_string(),
        Value::String(
            if hook_name == "PermissionRequest" {
                "PermissionRequest"
            } else {
                "PreToolUse"
            }
            .to_string(),
        ),
    );
    hook_specific.insert("updatedInput".to_string(), Value::Object(updated_input));

    if hook_name == "PermissionRequest" {
        hook_specific.insert("decision".to_string(), json!({ "behavior": "allow" }));
    } else {
        hook_specific.insert(
            "permissionDecision".to_string(),
            Value::String("allow".to_string()),
        );
        hook_specific.insert(
            "permissionDecisionReason".to_string(),
            Value::String("User answered the question".to_string()),
        );
    }

    json!({ "hookSpecificOutput": Value::Object(hook_specific) }).to_string()
}

fn copy_original_question_input(question: &QuestionData) -> Map<String, Value> {
    match &question.original_input {
        Some(Value::Object(map)) => map.clone(),
        _ => Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(event_name: &str) -> HookEvent {
        HookEvent {
            event_name: event_name.to_string(),
            session_id: None,
            tool_name: None,
            tool_use_id: None,
            agent_id: None,
            tool_input: None,
            raw_json: json!({}),
            source: Some("claude".to_string()),
            parent_pid: None,
            tracked_pid: None,
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        }
    }

    #[test]
    fn pre_tool_use_allow_uses_permission_decision() {
        let out = build_permission_allow_response(&event("PreToolUse"), false);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "allow");
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    }

    #[test]
    fn permission_request_deny_uses_decision_object() {
        let out = build_permission_deny_response(&event("PermissionRequest"), "nope");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(v["hookSpecificOutput"]["decision"]["reason"], "nope");
    }
}
