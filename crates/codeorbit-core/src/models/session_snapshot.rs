use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::{
    AgentStatus, ChatMessage, HookEvent, PermissionRequest, QuestionData, SideEffect,
    SupportedSource, ToolHistoryEntry,
};

/// 单个 AI 工具会话的快照状态
///
/// Rust 优势：使用 Clone derive 代替手动 Clone() 方法，
/// 所有字段都实现了 Clone，Rust 会自动生成正确的克隆逻辑。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    pub status: AgentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_tool_description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracked_process_started_at_utc: Option<DateTime<Utc>>,
    pub tool_history: Vec<ToolHistoryEntry>,
    pub recent_messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_user_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_assistant_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_text: Option<String>,
    pub interrupted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    pub transcript_position: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_session_id: Option<String>,
}

const MAX_TOOL_HISTORY_ENTRIES: usize = 50;
const MAX_RECENT_MESSAGES: usize = 6;

impl SessionSnapshot {
    pub fn new(session_id: String, source: String) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            source,
            project_name: None,
            working_directory: None,
            status: AgentStatus::Idle,
            current_tool_name: None,
            current_tool_description: None,
            created_at: now,
            last_updated_at: now,
            pid: 0,
            tracked_process_started_at_utc: None,
            tool_history: Vec::new(),
            recent_messages: Vec::new(),
            last_user_prompt: None,
            last_assistant_message: None,
            completion_text: None,
            interrupted: false,
            transcript_path: None,
            transcript_position: 0,
            terminal_app: None,
            terminal_session_id: None,
        }
    }

    /// 纯函数 reducer：根据事件计算新状态和副作用
    ///
    /// 这是 Rust 的优势所在：不可变数据 + 纯函数 = 易于测试和推理。
    /// C# 版本使用 Clone() 方法，Rust 使用 derive(Clone) 自动生成。
    pub fn reduce_event(current: Option<Self>, evt: &HookEvent) -> (Self, SideEffect) {
        let mut state = match current {
            Some(existing) => {
                let mut s = Self::apply_event_metadata(existing, evt);
                s.last_updated_at = Utc::now();
                s
            }
            None => {
                let session_id = evt
                    .session_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()[..8].to_string());
                let source = evt
                    .source
                    .as_ref()
                    .filter(|s| Self::is_known_source(s))
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
                let mut s = Self::new(session_id, source);
                s = Self::apply_event_metadata(s, evt);
                s
            }
        };

        let normalized_event = Self::normalize_event_name(&state.source, &evt.event_name);

        let (new_state, effect) = match normalized_event.as_str() {
            "UserPromptSubmit" => Self::handle_user_prompt_submit(state, evt),
            "PreToolUse" if Self::should_block_question_tool(evt) => {
                let sid = state.session_id.clone();
                let tool_name = evt
                    .tool_name
                    .clone()
                    .or_else(|| state.current_tool_name.clone());
                let description = Self::format_tool_description(evt);
                let question = Self::extract_question_data(&sid, evt);

                state.status = AgentStatus::WaitingQuestion;
                state.current_tool_name = tool_name;
                state.current_tool_description = description;
                state.last_updated_at = Utc::now();

                (
                    state,
                    SideEffect::ShowQuestionCard {
                        session_id: sid,
                        question,
                    },
                )
            }
            "PreToolUse" if Self::has_approval_needed_signal(evt) => {
                let sid = state.session_id.clone();
                let tool_name = evt
                    .tool_name
                    .clone()
                    .or_else(|| state.current_tool_name.clone());
                let description = Self::format_tool_description(evt);
                let permission = Self::build_permission_request(&state, evt, "PreToolUse");

                state.status = AgentStatus::WaitingApproval;
                state.current_tool_name = tool_name;
                state.current_tool_description = description;
                state.last_updated_at = Utc::now();

                (
                    state,
                    SideEffect::ShowApprovalCard {
                        session_id: sid,
                        request: permission,
                    },
                )
            }
            "PreToolUse" => {
                let tool_name = evt
                    .tool_name
                    .clone()
                    .or_else(|| state.current_tool_name.clone());
                let description = Self::format_tool_description(evt);

                state.status = AgentStatus::Running;
                state.current_tool_name = tool_name;
                state.current_tool_description = description;
                state.last_updated_at = Utc::now();

                (state, SideEffect::None)
            }
            "PostToolUse" => Self::handle_post_tool_use(state, evt, true),
            "PostToolUseFailure" => Self::handle_post_tool_use(state, evt, false),
            "PermissionDenied" => {
                state.status = AgentStatus::Processing;
                state.current_tool_name = None;
                state.current_tool_description = None;
                state.last_updated_at = Utc::now();
                (state, SideEffect::None)
            }
            "Stop" => Self::handle_stop(state, evt),
            "SessionEnd" => {
                state.status = AgentStatus::Idle;
                state.current_tool_name = None;
                state.current_tool_description = None;
                state.last_updated_at = Utc::now();
                (state, SideEffect::None)
            }
            "SessionStart" => Self::handle_session_start(state, evt),
            "SubagentStart" => {
                state.status = AgentStatus::Running;
                state.current_tool_name = Some("Agent".to_string());
                state.current_tool_description =
                    get_string_field(&evt.raw_json, &["agent_type", "agentType", "agent"]);
                state.last_updated_at = Utc::now();
                (state, SideEffect::None)
            }
            "SubagentStop" => {
                state.status = AgentStatus::Processing;
                state.current_tool_name = None;
                state.current_tool_description = None;
                state.last_updated_at = Utc::now();
                (state, SideEffect::None)
            }
            "PreCompact" => {
                state.status = AgentStatus::Running;
                state.current_tool_name = Some("Compact".to_string());
                state.current_tool_description = Some("压缩上下文".to_string());
                state.last_updated_at = Utc::now();
                (state, SideEffect::None)
            }
            "PostCompact" => {
                state.status = AgentStatus::Processing;
                state.current_tool_name = None;
                state.current_tool_description = None;
                state.last_updated_at = Utc::now();
                (state, SideEffect::None)
            }
            "PermissionRequest" if Self::should_block_question_tool(evt) => {
                let sid = state.session_id.clone();
                state.status = AgentStatus::WaitingQuestion;
                state.last_updated_at = Utc::now();
                let question = Self::extract_question_data(&sid, evt);
                (
                    state,
                    SideEffect::ShowQuestionCard {
                        session_id: sid,
                        question,
                    },
                )
            }
            "PermissionRequest" => {
                let sid = state.session_id.clone();
                state.status = AgentStatus::WaitingApproval;
                state.last_updated_at = Utc::now();
                let permission = Self::build_permission_request(&state, evt, "PermissionRequest");
                (
                    state,
                    SideEffect::ShowApprovalCard {
                        session_id: sid,
                        request: permission,
                    },
                )
            }
            event_name if Self::is_question_event(event_name, evt) => {
                let sid = state.session_id.clone();
                state.status = AgentStatus::WaitingQuestion;
                state.last_updated_at = Utc::now();
                let question = Self::extract_question_data(&sid, evt);
                (
                    state,
                    SideEffect::ShowQuestionCard {
                        session_id: sid,
                        question,
                    },
                )
            }
            _ => {
                state.last_updated_at = Utc::now();
                (state, SideEffect::None)
            }
        };

        (Self::apply_event_metadata(new_state, evt), effect)
    }

    fn apply_event_metadata(mut state: Self, evt: &HookEvent) -> Self {
        state.last_updated_at = Utc::now();

        if let Some(source) = &evt.source
            && Self::is_known_source(source)
        {
            state.source = source.clone();
        }

        if let Some(tracked_pid) = evt.tracked_pid {
            let previous_pid = state.pid;
            state.pid = tracked_pid;
            if let Some(started_at) = evt.tracked_process_started_at_utc {
                state.tracked_process_started_at_utc = Some(started_at);
            } else if previous_pid != tracked_pid {
                state.tracked_process_started_at_utc = None;
            }
        } else if let Some(parent_pid) = evt.parent_pid {
            state.pid = parent_pid;
            state.tracked_process_started_at_utc = None;
        }

        state
    }

    fn is_known_source(source: &str) -> bool {
        !source.eq_ignore_ascii_case("unknown") && SupportedSource::is_valid(source)
    }

    fn normalize_event_name(_source: &str, event_name: &str) -> String {
        // ponytail: 当前直接返回，未来可以根据 source 进行事件名映射
        event_name.to_string()
    }

    fn handle_user_prompt_submit(mut state: Self, evt: &HookEvent) -> (Self, SideEffect) {
        state.status = AgentStatus::Processing;
        state.current_tool_name = None;
        state.current_tool_description = None;

        let prompt = first_string_from_event(
            evt,
            &[
                "prompt",
                "user_prompt",
                "userPrompt",
                "message",
                "input",
                "content",
                "text",
            ],
        );

        if let Some(prompt) = prompt
            && !Self::is_system_placeholder_prompt(&prompt)
        {
            state.last_user_prompt = Some(prompt.clone());
            state.completion_text = None;
            state.last_assistant_message = None;
            Self::add_recent_message(&mut state, ChatMessage::new(true, &prompt));
        }

        state.last_updated_at = Utc::now();
        (state, SideEffect::None)
    }

    fn handle_post_tool_use(mut state: Self, evt: &HookEvent, success: bool) -> (Self, SideEffect) {
        let tool_name = evt
            .tool_name
            .clone()
            .or_else(|| state.current_tool_name.clone());
        if let Some(tool_name) = tool_name {
            let entry = ToolHistoryEntry {
                tool_name,
                timestamp: Utc::now(),
                description: Self::format_tool_description(evt)
                    .or_else(|| state.current_tool_description.clone()),
                success,
            };
            Self::add_tool_history(&mut state, entry);
        }

        state.status = AgentStatus::Processing;
        state.current_tool_name = None;
        state.current_tool_description = None;
        state.last_updated_at = Utc::now();

        (state, SideEffect::None)
    }

    fn handle_stop(mut state: Self, evt: &HookEvent) -> (Self, SideEffect) {
        state.status = AgentStatus::Idle;
        state.current_tool_name = None;
        state.current_tool_description = None;

        let stop_reason = get_string_field(&evt.raw_json, &["stop_reason", "stopReason", "reason"]);
        state.interrupted = stop_reason
            .as_ref()
            .map(|r| r.eq_ignore_ascii_case("user") || r.eq_ignore_ascii_case("interrupted"))
            .unwrap_or(false);

        let assistant_message = first_string_from_event(
            evt,
            &[
                "last_assistant_message",
                "lastAssistantMessage",
                "text",
                "message",
                "summary",
            ],
        );

        if let Some(msg) = assistant_message {
            state.last_assistant_message = Some(msg.clone());
            state.completion_text = Some(msg.clone());
            Self::add_recent_message(&mut state, ChatMessage::new(false, &msg));
        } else {
            state.completion_text = state.last_assistant_message.clone();
        }

        state.last_updated_at = Utc::now();

        if state.completion_text.is_some() {
            (
                state,
                SideEffect::PlaySound {
                    sound_name: "complete".to_string(),
                },
            )
        } else {
            (state, SideEffect::None)
        }
    }

    fn handle_session_start(mut state: Self, evt: &HookEvent) -> (Self, SideEffect) {
        state = Self::apply_event_metadata(state, evt);
        state.status = AgentStatus::Idle;
        state.last_updated_at = Utc::now();

        if let Some(term_app) = get_string_field(&evt.raw_json, &["_term_app"]) {
            state.terminal_app = Some(term_app);
        }

        if let Some(iterm) = get_string_field(&evt.raw_json, &["_iterm_session"]) {
            state.terminal_session_id = Some(iterm);
        } else if let Some(wt) = get_string_field(&evt.raw_json, &["WT_SESSION", "_wt_session"]) {
            state.terminal_session_id = Some(wt);
        }

        (
            state,
            SideEffect::PlaySound {
                sound_name: "start".to_string(),
            },
        )
    }

    fn is_system_placeholder_prompt(prompt: &str) -> bool {
        let trimmed = prompt.trim_start();
        [
            "<local-command-stdout>",
            "<local-command-stderr>",
            "<command-name>",
            "<command-message>",
            "<command-args>",
        ]
        .iter()
        .any(|marker| trimmed.starts_with(marker))
    }

    pub fn add_recent_message(state: &mut Self, message: ChatMessage) {
        if message.text.trim().is_empty() {
            return;
        }

        if message.is_user && Self::is_system_placeholder_prompt(&message.text) {
            return;
        }

        // 去重：如果最后一条消息和当前消息相同，跳过
        if let Some(last) = state.recent_messages.last()
            && last.is_user == message.is_user
            && last.text == message.text
        {
            return;
        }

        state.recent_messages.push(message.clone());

        // 保持最多 MAX_RECENT_MESSAGES 条
        while state.recent_messages.len() > MAX_RECENT_MESSAGES {
            state.recent_messages.remove(0);
        }

        if message.is_user {
            state.last_user_prompt = Some(message.text);
        } else {
            state.last_assistant_message = Some(message.text);
        }
    }

    fn add_tool_history(state: &mut Self, entry: ToolHistoryEntry) {
        state.tool_history.push(entry);
        while state.tool_history.len() > MAX_TOOL_HISTORY_ENTRIES {
            state.tool_history.remove(0);
        }
    }

    fn format_tool_description(evt: &HookEvent) -> Option<String> {
        let tool_input = evt.tool_input.as_ref()?;
        let tool_name = evt.tool_name.as_ref()?;

        match tool_name.as_str() {
            "Bash" => tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .map(String::from),
            "Read" | "Edit" | "Write" => tool_input
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(String::from),
            "Grep" | "Glob" => tool_input
                .get("pattern")
                .and_then(|v| v.as_str())
                .map(String::from),
            _ => None,
        }
    }

    fn build_permission_request(
        state: &Self,
        evt: &HookEvent,
        hook_event_name: &str,
    ) -> PermissionRequest {
        PermissionRequest {
            session_id: state.session_id.clone(),
            tool_name: evt
                .tool_name
                .clone()
                .or_else(|| state.current_tool_name.clone())
                .unwrap_or_default(),
            tool_use_id: evt.tool_use_id.clone(),
            tool_input: Self::extract_tool_input_dictionary(evt),
            description: Self::format_tool_description(evt)
                .or_else(|| state.current_tool_description.clone()),
            hook_event_name: hook_event_name.to_string(),
        }
    }

    fn extract_tool_input_dictionary(
        evt: &HookEvent,
    ) -> Option<std::collections::HashMap<String, serde_json::Value>> {
        let tool_input = evt.tool_input.as_ref()?;
        let obj = tool_input.as_object()?;

        let mut result = std::collections::HashMap::new();
        for (key, value) in obj {
            result.insert(key.clone(), value.clone());
        }
        Some(result)
    }

    fn should_block_question_tool(evt: &HookEvent) -> bool {
        // ponytail: 简化实现，检查是否是 AskUserQuestion 工具
        evt.tool_name
            .as_ref()
            .map(|name| name == "AskUserQuestion" || name == "RequestUserInput")
            .unwrap_or(false)
    }

    fn has_approval_needed_signal(evt: &HookEvent) -> bool {
        Self::contains_approval_needed_signal(&evt.tool_input)
            || Self::contains_approval_needed_signal_value(&evt.raw_json)
    }

    fn contains_approval_needed_signal(element: &Option<serde_json::Value>) -> bool {
        let Some(value) = element else { return false };
        Self::contains_approval_needed_signal_value(value)
    }

    fn contains_approval_needed_signal_value(value: &serde_json::Value) -> bool {
        let Some(obj) = value.as_object() else {
            return false;
        };

        for (key, val) in obj {
            if Self::is_approval_signal_name(key) && Self::is_truthy_approval_signal(val) {
                return true;
            }
            if val.is_object() && Self::contains_approval_needed_signal_value(val) {
                return true;
            }
        }

        false
    }

    fn is_approval_signal_name(name: &str) -> bool {
        matches!(
            name,
            "permission_request"
                | "permissionRequest"
                | "requires_approval"
                | "requiresApproval"
                | "approval_required"
                | "approvalRequired"
        )
    }

    fn is_truthy_approval_signal(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Bool(true) => true,
            serde_json::Value::String(s) => !s.eq_ignore_ascii_case("false") && s != "0",
            serde_json::Value::Object(_) => true,
            _ => false,
        }
    }

    fn is_question_event(normalized_event: &str, evt: &HookEvent) -> bool {
        (normalized_event == "Notification" || normalized_event.contains("Question"))
            && (Self::contains_question(&evt.tool_input)
                || Self::contains_question_value(&evt.raw_json))
    }

    fn contains_question(element: &Option<serde_json::Value>) -> bool {
        let Some(value) = element else { return false };
        Self::contains_question_value(value)
    }

    fn contains_question_value(value: &serde_json::Value) -> bool {
        let Some(obj) = value.as_object() else {
            return false;
        };
        obj.contains_key("question") || obj.contains_key("questions")
    }

    fn extract_question_data(session_id: &str, _evt: &HookEvent) -> QuestionData {
        // ponytail: 简化实现，返回空问题数据
        // 完整实现需要在 Task 1.6 (Core Services) 中完成
        QuestionData {
            session_id: session_id.to_string(),
            ..Default::default()
        }
    }
}

fn first_string_from_event(evt: &HookEvent, keys: &[&str]) -> Option<String> {
    first_string_from_element(&evt.raw_json, keys).or_else(|| {
        evt.tool_input
            .as_ref()
            .and_then(|ti| first_string_from_element(ti, keys))
    })
}

fn first_string_from_element(element: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let obj = element.as_object()?;

    for key in keys {
        if let Some(value) = obj.get(*key)
            && let Some(s) = value.as_str()
        {
            return Some(s.to_string());
        }
    }

    for nest_key in &[
        "message",
        "payload",
        "data",
        "input",
        "params",
        "tool_input",
    ] {
        if let Some(nested) = obj.get(*nest_key)
            && nested.is_object()
            && let Some(result) = first_string_from_element(nested, keys)
            && !result.trim().is_empty()
        {
            return Some(result);
        }
    }

    None
}

fn get_string_field(json: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = json.get(*key)
            && let Some(s) = value.as_str()
        {
            return Some(s.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_new_session() {
        let session = SessionSnapshot::new("test123".to_string(), "claude".to_string());
        assert_eq!(session.session_id, "test123");
        assert_eq!(session.source, "claude");
        assert_eq!(session.status, AgentStatus::Idle);
    }

    #[test]
    fn test_reduce_user_prompt() {
        let evt = HookEvent::from_json(
            &json!({
                "hook_event_name": "UserPromptSubmit",
                "session_id": "s1",
                "prompt": "Hello world"
            }),
            Some("claude"),
        )
        .unwrap();

        let (state, effect) = SessionSnapshot::reduce_event(None, &evt);
        assert_eq!(state.status, AgentStatus::Processing);
        assert_eq!(state.last_user_prompt, Some("Hello world".to_string()));
        assert!(matches!(effect, SideEffect::None));
    }

    #[test]
    fn test_reduce_stop_event() {
        let evt = HookEvent::from_json(
            &json!({
                "hook_event_name": "Stop",
                "session_id": "s1",
                "last_assistant_message": "Done!"
            }),
            Some("claude"),
        )
        .unwrap();

        let (state, effect) = SessionSnapshot::reduce_event(None, &evt);
        assert_eq!(state.status, AgentStatus::Idle);
        assert_eq!(state.last_assistant_message, Some("Done!".to_string()));
        assert!(matches!(effect, SideEffect::PlaySound { .. }));
    }
}
