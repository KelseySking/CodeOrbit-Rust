//! Codex 风格 Hook 响应格式构建器

use serde_json::{Map, Value, json};

use super::codex_permission_rules;
use super::event_normalizer::normalize_event_name;
use super::legacy_question_response_builder::Answers;
use crate::models::{HookEvent, PermissionRequest};

pub(crate) fn build_permission_allow_response(
    evt: &HookEvent,
    request: Option<&PermissionRequest>,
    always: bool,
) -> String {
    if always && let Some(req) = request {
        codex_permission_rules::try_append_allow_rule(req);
    }
    build_approval_decision_response(evt, "allow", None, always)
}

pub(crate) fn build_permission_deny_response(evt: &HookEvent, reason: &str) -> String {
    build_approval_decision_response(evt, "deny", Some(reason), false)
}

pub(crate) fn build_request_user_input_answer_response(
    evt: &HookEvent,
    answers: &Answers,
) -> String {
    if hook_event_name(evt) == "PreToolUse" {
        return build_pre_tool_use_deny_response(&build_request_user_input_answer_reason(answers));
    }
    build_request_user_input_decision_response(evt, "allow", None)
}

pub(crate) fn build_request_user_input_dismiss_response(evt: &HookEvent, reason: &str) -> String {
    if hook_event_name(evt) == "PreToolUse" {
        build_pre_tool_use_deny_response(&build_request_user_input_dismiss_reason(reason))
    } else {
        build_request_user_input_decision_response(evt, "deny", Some(reason))
    }
}

fn build_request_user_input_answer_reason(answers: &Answers) -> String {
    let response = json!({ "answers": build_request_user_input_answers(answers) });
    format!("CodeOrbit HUD answer: {response}")
}

fn build_request_user_input_dismiss_reason(reason: &str) -> String {
    if reason.trim().is_empty() {
        "User dismissed request_user_input in CodeOrbit HUD.".to_string()
    } else {
        format!("User dismissed request_user_input in CodeOrbit HUD: {reason}")
    }
}

fn build_request_user_input_answers(answers: &Answers) -> Value {
    let mut answer_object = Map::new();
    for (key, values) in answers {
        answer_object.insert(key.clone(), json!({ "answers": values.to_vec() }));
    }
    Value::Object(answer_object)
}

fn build_approval_decision_response(
    evt: &HookEvent,
    behavior: &str,
    reason: Option<&str>,
    _always: bool,
) -> String {
    if hook_event_name(evt) == "PreToolUse" {
        return if behavior.eq_ignore_ascii_case("deny") {
            build_pre_tool_use_deny_response(reason.unwrap_or("User denied this operation"))
        } else {
            "{}".to_string()
        };
    }
    build_request_user_input_decision_response(evt, behavior, reason)
}

fn build_pre_tool_use_deny_response(reason: &str) -> String {
    let reason = if reason.trim().is_empty() {
        "Denied by CodeOrbit HUD"
    } else {
        reason
    };
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    })
    .to_string()
}

fn build_request_user_input_decision_response(
    evt: &HookEvent,
    behavior: &str,
    reason: Option<&str>,
) -> String {
    let mut decision = Map::new();
    decision.insert("behavior".to_string(), Value::String(behavior.to_string()));
    if let Some(r) = reason
        && !r.trim().is_empty()
    {
        decision.insert("reason".to_string(), Value::String(r.to_string()));
    }

    json!({
        "hookSpecificOutput": {
            "hookEventName": hook_event_name(evt),
            "decision": Value::Object(decision),
        }
    })
    .to_string()
}

fn hook_event_name(evt: &HookEvent) -> &'static str {
    let source = evt.source.as_deref().unwrap_or("unknown");
    if normalize_event_name(source, &evt.event_name) == "PreToolUse" {
        "PreToolUse"
    } else {
        "PermissionRequest"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(event_name: &str) -> HookEvent {
        HookEvent {
            event_name: event_name.to_string(),
            session_id: None,
            tool_name: None,
            tool_use_id: None,
            agent_id: None,
            tool_input: None,
            raw_json: json!({}),
            source: Some("codex".to_string()),
            parent_pid: None,
            tracked_pid: None,
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        }
    }

    #[test]
    fn pre_tool_use_allow_returns_empty_object() {
        let out = build_permission_allow_response(&event("PreToolUse"), None, false);
        assert_eq!(out, "{}");
    }

    #[test]
    fn pre_tool_use_deny_has_permission_decision() {
        let out = build_permission_deny_response(&event("PreToolUse"), "blocked");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecisionReason"],
            "blocked"
        );
    }
}
