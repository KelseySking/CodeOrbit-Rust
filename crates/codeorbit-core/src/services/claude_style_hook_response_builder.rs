//! Claude 风格 Hook 响应格式构建器

use serde_json::{json, Map, Value};

use super::event_normalizer::normalize_event_name;
use super::legacy_question_response_builder::Answers;
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
    updated_input.insert(
        "answers".to_string(),
        Value::Object(claude_shaped_answers(question, answers)),
    );

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

/// Claude Code 2.x 只认 `answers: { "<问题原文>": "选项1,选项2" }`。
/// 前端按 index / id 回传时必须重映射，否则工具结果是空的，回合锁死。
/// 多选无空格逗号拼接，和 Claude 自己的 join 一致。
fn claude_shaped_answers(question: &QuestionData, answers: &Answers) -> Map<String, Value> {
    let items = question.questions.as_deref().unwrap_or(&[]);
    let mut shaped = Map::new();
    for (i, (key, values)) in answers.iter().enumerate() {
        let text = question_text_for_key(question, items, key, i);
        if text.is_empty() || values.is_empty() {
            continue;
        }
        shaped.insert(text, Value::String(values.join(",")));
    }
    shaped
}

fn question_text_for_key(
    question: &QuestionData,
    items: &[crate::models::QuestionItem],
    key: &str,
    fallback_index: usize,
) -> String {
    if let Some(item) = items.iter().find(|item| item.id.as_deref() == Some(key))
        && !item.question.is_empty()
    {
        return item.question.clone();
    }
    if let Ok(index) = key.parse::<usize>()
        && let Some(item) = items.get(index)
        && !item.question.is_empty()
    {
        return item.question.clone();
    }
    if let Some(item) = items.get(fallback_index)
        && !item.question.is_empty()
        && (key == item.question || key.parse::<usize>().is_ok() || items.len() == 1)
    {
        return item.question.clone();
    }
    if items.is_empty() && !question.question.is_empty() {
        return question.question.clone();
    }
    if !key.is_empty() && key.parse::<usize>().is_err() {
        return key.to_string();
    }
    String::new()
}

pub(crate) fn build_question_deny_response(evt: &HookEvent, reason: &str) -> String {
    let hook_name = normalize_event_name(source_of(evt), &evt.event_name);
    if hook_name == "PermissionRequest" {
        return build_permission_deny_response(evt, reason);
    }
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    })
    .to_string()
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

    fn question_with_items(items: Vec<(&str, &str)>) -> QuestionData {
        QuestionData {
            is_ask_user_question: true,
            questions: Some(
                items
                    .into_iter()
                    .map(|(id, text)| crate::models::QuestionItem {
                        id: Some(id.to_string()),
                        question: text.to_string(),
                        header: None,
                        options: None,
                        multi_select: false,
                        allow_free_text: false,
                    })
                    .collect(),
            ),
            ..QuestionData::default()
        }
    }

    #[test]
    fn ask_user_question_answers_are_keyed_by_question_text() {
        let q = question_with_items(vec![("q0", "Pick a color?")]);
        let out = build_ask_user_question_answer_response(
            &event("PreToolUse"),
            &q,
            &[("0".to_string(), vec!["red".to_string(), "blue".to_string()])],
        );
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v["hookSpecificOutput"]["updatedInput"]["answers"]["Pick a color?"],
            "red,blue"
        );
        assert!(v["hookSpecificOutput"]["updatedInput"]["answers"].get("0").is_none());
    }

    #[test]
    fn question_deny_uses_hook_specific_output() {
        let out = build_question_deny_response(&event("PreToolUse"), "timeout");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(v["hookSpecificOutput"]["permissionDecisionReason"], "timeout");
    }
}
