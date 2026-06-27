//! Hook 响应构建分派 — 按来源的 permission 风格选择 Claude / Codex / 旧版构建器

use super::legacy_question_response_builder::Answers;
use super::{
    claude_style_hook_response_builder as claude, codex_hook_response_builder as codex,
    hook_tool_classifier, legacy_question_response_builder as legacy,
};
use crate::models::{HookEvent, PermissionRequest, QuestionData};
use crate::sources::adapter_registry;
use crate::sources::plugin_models::PermissionResponseStyle;

fn is_codex_style(evt: &HookEvent) -> bool {
    adapter_registry::global()
        .get(evt.source.as_deref())
        .permission_response_style()
        == PermissionResponseStyle::Codex
}

pub fn build_permission_allow_response(
    evt: &HookEvent,
    request: Option<&PermissionRequest>,
    always: bool,
) -> String {
    if is_codex_style(evt) {
        codex::build_permission_allow_response(evt, request, always)
    } else {
        claude::build_permission_allow_response(evt, always)
    }
}

pub fn build_permission_deny_response(evt: &HookEvent, reason: &str) -> String {
    if is_codex_style(evt) {
        codex::build_permission_deny_response(evt, reason)
    } else {
        claude::build_permission_deny_response(evt, reason)
    }
}

pub fn build_question_answer_response(
    evt: &HookEvent,
    question: &QuestionData,
    answers: &Answers,
) -> String {
    if question.is_codex_request_user_input
        || hook_tool_classifier::is_codex_request_user_input(evt)
    {
        return codex::build_request_user_input_answer_response(evt, answers);
    }
    if question.is_ask_user_question || hook_tool_classifier::is_ask_user_question(evt) {
        return claude::build_ask_user_question_answer_response(evt, question, answers);
    }
    legacy::build_question_answer_response(question, answers)
}

pub fn build_question_dismiss_response(evt: &HookEvent, reason: &str) -> String {
    if hook_tool_classifier::is_codex_request_user_input(evt) {
        codex::build_request_user_input_dismiss_response(evt, reason)
    } else {
        legacy::build_question_dismiss_response(reason)
    }
}
