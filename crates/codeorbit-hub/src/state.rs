//! HubState — 中央状态机：会话、待处理操作（权限/问题）生命周期与实时事件广播

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use tokio::sync::{RwLock, broadcast, oneshot};

use codeorbit_contracts::{
    ChatMessageDto, HubEventDto, PendingActionDto, PendingResolutionDto, PermissionRequestDto,
    QuestionAnswerRequest, QuestionDto, QuestionItemDto, QuestionOptionDto, SessionDto,
    ToolHistoryEntryDto,
};
use codeorbit_core::models::{
    ChatMessage, HookEvent, PermissionRequest, QuestionData, QuestionItem, QuestionOption,
    SessionSnapshot, SideEffect, SupportedSource, ToolHistoryEntry,
};
use codeorbit_core::services::hook_response_builder;
use codeorbit_core::services::transcript_message_reader::read_new_messages;
use codeorbit_core::services::{hook_tool_classifier, normalize_event_name};

const MAX_HISTORY_ENTRIES: usize = 200;
const REALTIME_CHANNEL_CAPACITY: usize = 256;

/// 自动审批判定函数
pub type AutoApprove = Box<dyn Fn(&PermissionRequest) -> bool + Send + Sync>;

/// 队列中的待处理权限请求
struct PendingPermission {
    action_id: String,
    created_at: DateTime<Utc>,
    request: PermissionRequest,
    completion: oneshot::Sender<String>,
    event: HookEvent,
}

/// 队列中的待处理问题
struct PendingQuestion {
    action_id: String,
    created_at: DateTime<Utc>,
    question: QuestionData,
    completion: oneshot::Sender<String>,
    event: HookEvent,
    current_question_index: i32,
    answers: Vec<(String, Vec<String>)>,
}

impl PendingQuestion {
    fn current_item(&self) -> Option<&QuestionItem> {
        let questions = self.question.questions.as_ref()?;
        if questions.is_empty() {
            return None;
        }
        let idx = self
            .current_question_index
            .clamp(0, questions.len() as i32 - 1) as usize;
        questions.get(idx)
    }

    fn current_question_text(&self) -> String {
        self.current_item()
            .map(|i| i.question.clone())
            .unwrap_or_else(|| self.question.question.clone())
    }

    fn current_answer_key(&self) -> String {
        if let Some(item) = self.current_item()
            && let Some(id) = &item.id
        {
            return id.clone();
        }
        self.question
            .id
            .clone()
            .unwrap_or_else(|| self.current_question_text())
    }
}

/// 阻塞事件处理结果
pub enum BlockingOutcome {
    /// 立即响应（自动审批或无需卡片）
    Immediate(String),
    /// 等待用户决策
    Pending(Box<PendingHandle>),
}

/// 待决策句柄：交给异步等待方
pub struct PendingHandle {
    pub action_id: String,
    pub session_id: Option<String>,
    pub kind: &'static str,
    pub event: HookEvent,
    rx: oneshot::Receiver<String>,
}

/// 中央状态机
pub struct HubState {
    sessions: HashMap<String, SessionSnapshot>,
    permission_queue: VecDeque<PendingPermission>,
    question_queue: VecDeque<PendingQuestion>,
    history: VecDeque<PendingResolutionDto>,
    should_auto_approve: Option<AutoApprove>,
    events: broadcast::Sender<HubEventDto>,
}

impl HubState {
    pub fn new() -> Self {
        Self::with_auto_approve(None)
    }

    pub fn with_auto_approve(should_auto_approve: Option<AutoApprove>) -> Self {
        let (events, _) = broadcast::channel(REALTIME_CHANNEL_CAPACITY);
        Self {
            sessions: HashMap::new(),
            permission_queue: VecDeque::new(),
            question_queue: VecDeque::new(),
            history: VecDeque::new(),
            should_auto_approve,
            events,
        }
    }

    /// 订阅实时事件广播
    pub fn subscribe(&self) -> broadcast::Receiver<HubEventDto> {
        self.events.subscribe()
    }

    /// 主动广播一个实时事件（供 sources 等外部操作使用）
    pub fn publish(&self, event_type: &str, data: Option<Value>) {
        let _ = self.events.send(HubEventDto {
            event_type: event_type.to_string(),
            timestamp_utc: Utc::now(),
            data,
        });
    }

    /// 所有会话跟踪的非零 PID（供进程监控使用）
    pub fn tracked_pids(&self) -> Vec<u32> {
        self.sessions
            .values()
            .map(|s| s.pid)
            .filter(|pid| *pid != 0)
            .collect()
    }

    // ---------- 查询 ----------

    pub fn get_sessions(&self) -> Vec<SessionDto> {
        self.sessions.values().map(map_session).collect()
    }

    pub fn get_session(&self, session_id: &str) -> Option<SessionDto> {
        self.sessions.get(session_id).map(map_session)
    }

    pub fn get_session_messages(&self, session_id: &str) -> Vec<ChatMessageDto> {
        self.sessions
            .get(session_id)
            .map(|s| s.recent_messages.iter().map(map_message).collect())
            .unwrap_or_default()
    }

    pub fn get_pending_actions(&self) -> Vec<PendingActionDto> {
        let mut actions: Vec<(DateTime<Utc>, PendingActionDto)> =
            Vec::with_capacity(self.permission_queue.len() + self.question_queue.len());
        for p in &self.permission_queue {
            actions.push((p.created_at, self.map_permission_action(p)));
        }
        for q in &self.question_queue {
            actions.push((q.created_at, self.map_question_action(q)));
        }
        actions.sort_by_key(|(created, _)| *created);
        actions.into_iter().map(|(_, dto)| dto).collect()
    }

    pub fn get_pending_action(&self, action_id: &str) -> Option<PendingActionDto> {
        if let Some(p) = self
            .permission_queue
            .iter()
            .find(|p| p.action_id == action_id)
        {
            return Some(self.map_permission_action(p));
        }
        self.question_queue
            .iter()
            .find(|q| q.action_id == action_id)
            .map(|q| self.map_question_action(q))
    }

    pub fn get_pending_history(&self, limit: usize) -> Vec<PendingResolutionDto> {
        if limit == 0 {
            return Vec::new();
        }
        let len = self.history.len();
        let start = len.saturating_sub(limit);
        self.history.iter().skip(start).cloned().collect()
    }

    // ---------- 事件处理 ----------

    /// 非阻塞事件
    pub fn handle_event(&mut self, evt: &HookEvent) {
        let (session_id, normalized, effect) = self.apply_event(evt);
        let rt = to_realtime_event_type(normalized.as_deref(), &effect);
        self.emit(session_id.as_deref(), None, rt, None);
    }

    /// 阻塞事件第一阶段：应用事件、创建待处理操作（如需要）
    pub fn begin_blocking_event(&mut self, evt: HookEvent) -> BlockingOutcome {
        let (session_id, normalized, effect) = self.apply_event(&evt);
        let rt = to_realtime_event_type(normalized.as_deref(), &effect);

        let outcome = match &effect {
            SideEffect::ShowApprovalCard { request, .. } => {
                let auto = self
                    .should_auto_approve
                    .as_ref()
                    .map(|f| f(request))
                    .unwrap_or(false);
                if auto {
                    BlockingOutcome::Immediate(
                        hook_response_builder::build_permission_allow_response(
                            &evt,
                            Some(request),
                            false,
                        ),
                    )
                } else {
                    let (tx, rx) = oneshot::channel();
                    let action_id = new_action_id("permission");
                    let sid = request.session_id.clone();
                    self.permission_queue.push_back(PendingPermission {
                        action_id: action_id.clone(),
                        created_at: Utc::now(),
                        request: request.clone(),
                        completion: tx,
                        event: evt.clone(),
                    });
                    BlockingOutcome::Pending(Box::new(PendingHandle {
                        action_id,
                        session_id: Some(sid),
                        kind: "permission",
                        event: evt.clone(),
                        rx,
                    }))
                }
            }
            SideEffect::ShowQuestionCard { question, .. } => {
                let (tx, rx) = oneshot::channel();
                let action_id = new_action_id("question");
                let sid = question.session_id.clone();
                self.question_queue.push_back(PendingQuestion {
                    action_id: action_id.clone(),
                    created_at: Utc::now(),
                    question: question.clone(),
                    completion: tx,
                    event: evt.clone(),
                    current_question_index: 0,
                    answers: Vec::new(),
                });
                BlockingOutcome::Pending(Box::new(PendingHandle {
                    action_id,
                    session_id: Some(sid),
                    kind: "question",
                    event: evt.clone(),
                    rx,
                }))
            }
            // 无卡片效果：等同于超时拒绝/关闭（mirror C# 行为，但即时返回）
            _ => BlockingOutcome::Immediate(build_timeout_response(&evt)),
        };

        self.emit(session_id.as_deref(), None, rt, None);
        outcome
    }

    /// 阻塞超时处理：移除待处理操作并记录，返回拒绝/关闭响应
    pub fn resolve_timeout(
        &mut self,
        action_id: &str,
        session_id: Option<&str>,
        kind: &str,
        evt: &HookEvent,
    ) -> String {
        let removed = match kind {
            "permission" => self.take_permission(action_id).is_some(),
            "question" => self.take_question(action_id).is_some(),
            _ => false,
        };

        if removed {
            let resolution = PendingResolutionDto {
                action_id: action_id.to_string(),
                kind: kind.to_string(),
                session_id: session_id.map(str::to_string),
                source: None,
                decision: "timeout".to_string(),
                actor: None,
                reason: Some("timeout".to_string()),
                resolved_at_utc: Utc::now(),
            };
            self.record_history(resolution.clone());
            self.emit(
                session_id,
                Some(action_id),
                "pending.resolved",
                Some(&resolution),
            );
        } else {
            self.emit(session_id, None, "pending.updated", None);
        }

        build_timeout_response(evt)
    }

    // ---------- 解析（API 调用） ----------

    pub fn allow_permission(
        &mut self,
        action_id: &str,
        always: bool,
        actor: Option<String>,
    ) -> bool {
        let Some(pending) = self.take_permission(action_id) else {
            return false;
        };
        let source = self.session_source_key(&pending.request.session_id);
        let resolution = PendingResolutionDto {
            action_id: pending.action_id.clone(),
            kind: "permission".to_string(),
            session_id: Some(pending.request.session_id.clone()),
            source: Some(source),
            decision: if always { "allow-always" } else { "allow" }.to_string(),
            actor,
            reason: None,
            resolved_at_utc: Utc::now(),
        };
        self.record_history(resolution.clone());

        let response = hook_response_builder::build_permission_allow_response(
            &pending.event,
            Some(&pending.request),
            always,
        );
        let _ = pending.completion.send(response);
        self.emit(
            Some(&pending.request.session_id),
            Some(&pending.action_id),
            "pending.resolved",
            Some(&resolution),
        );
        true
    }

    pub fn deny_permission(
        &mut self,
        action_id: &str,
        reason: &str,
        actor: Option<String>,
    ) -> bool {
        let Some(pending) = self.take_permission(action_id) else {
            return false;
        };
        let source = self.session_source_key(&pending.request.session_id);
        let resolution = PendingResolutionDto {
            action_id: pending.action_id.clone(),
            kind: "permission".to_string(),
            session_id: Some(pending.request.session_id.clone()),
            source: Some(source),
            decision: "deny".to_string(),
            actor,
            reason: Some(reason.to_string()),
            resolved_at_utc: Utc::now(),
        };
        self.record_history(resolution.clone());

        let response =
            hook_response_builder::build_permission_deny_response(&pending.event, reason);
        let _ = pending.completion.send(response);
        self.emit(
            Some(&pending.request.session_id),
            Some(&pending.action_id),
            "pending.resolved",
            Some(&resolution),
        );
        true
    }

    pub fn answer_question(&mut self, action_id: &str, request: QuestionAnswerRequest) -> bool {
        let actor = request.actor;
        if let Some(map) = request.answers.filter(|m| !m.is_empty()) {
            let ordered: Vec<(String, Vec<String>)> = map.into_iter().collect();
            return self.resolve_question_with_answers(action_id, ordered, actor);
        }
        let single = match request.answer {
            Some(a) if !a.trim().is_empty() => vec![a],
            _ => vec![],
        };
        self.answer_current_question(action_id, single, actor).0
    }

    /// 回答当前子问题，返回 (是否找到, 是否已全部解析)
    pub fn answer_current_question(
        &mut self,
        action_id: &str,
        answers: Vec<String>,
        actor: Option<String>,
    ) -> (bool, bool) {
        let Some(pos) = self
            .question_queue
            .iter()
            .position(|q| q.action_id == action_id)
        else {
            return (false, false);
        };

        // 记录当前答案
        {
            let pending = &mut self.question_queue[pos];
            let cleaned = clean_answers(&answers);
            if cleaned.is_empty() {
                return (false, false);
            }
            let mut key = pending.current_answer_key();
            if key.trim().is_empty() {
                key = "answer".to_string();
            }
            set_answer(&mut pending.answers, &key, cleaned);
        }

        // 是否还有下一个子问题
        let advanced = advance_question_if_needed(&mut self.question_queue[pos]);
        if advanced {
            let sid = self.question_queue[pos].question.session_id.clone();
            self.emit(Some(&sid), Some(action_id), "pending.updated", None);
            return (true, false);
        }

        let pending = self.question_queue.remove(pos).expect("position valid");
        let source = self.session_source_key(&pending.question.session_id);
        let resolution = PendingResolutionDto {
            action_id: pending.action_id.clone(),
            kind: "question".to_string(),
            session_id: Some(pending.question.session_id.clone()),
            source: Some(source),
            decision: "answered".to_string(),
            actor,
            reason: None,
            resolved_at_utc: Utc::now(),
        };
        let response = hook_response_builder::build_question_answer_response(
            &pending.event,
            &pending.question,
            &pending.answers,
        );
        let _ = pending.completion.send(response);
        self.emit(
            Some(&pending.question.session_id),
            Some(action_id),
            "pending.resolved",
            Some(&resolution),
        );
        (true, true)
    }

    fn resolve_question_with_answers(
        &mut self,
        action_id: &str,
        answers: Vec<(String, Vec<String>)>,
        actor: Option<String>,
    ) -> bool {
        let Some(pos) = self
            .question_queue
            .iter()
            .position(|q| q.action_id == action_id)
        else {
            return false;
        };

        {
            let pending = &mut self.question_queue[pos];
            for (key, values) in &answers {
                let cleaned = clean_answers(values);
                if !cleaned.is_empty() {
                    set_answer(&mut pending.answers, key, cleaned);
                }
            }
            if pending.answers.is_empty() {
                return false;
            }
        }

        let pending = self.question_queue.remove(pos).expect("position valid");
        let source = self.session_source_key(&pending.question.session_id);
        let resolution = PendingResolutionDto {
            action_id: pending.action_id.clone(),
            kind: "question".to_string(),
            session_id: Some(pending.question.session_id.clone()),
            source: Some(source),
            decision: "answered".to_string(),
            actor,
            reason: None,
            resolved_at_utc: Utc::now(),
        };
        self.record_history(resolution.clone());

        let response = hook_response_builder::build_question_answer_response(
            &pending.event,
            &pending.question,
            &pending.answers,
        );
        let _ = pending.completion.send(response);
        self.emit(
            Some(&pending.question.session_id),
            Some(action_id),
            "pending.resolved",
            Some(&resolution),
        );
        true
    }

    pub fn dismiss_question(
        &mut self,
        action_id: &str,
        reason: &str,
        actor: Option<String>,
    ) -> bool {
        let Some(pending) = self.take_question(action_id) else {
            return false;
        };
        let source = self.session_source_key(&pending.question.session_id);
        let response =
            hook_response_builder::build_question_dismiss_response(&pending.event, reason);
        let _ = pending.completion.send(response);
        let resolution = PendingResolutionDto {
            action_id: pending.action_id.clone(),
            kind: "question".to_string(),
            session_id: Some(pending.question.session_id.clone()),
            source: Some(source),
            decision: "dismissed".to_string(),
            actor,
            reason: Some(reason.to_string()),
            resolved_at_utc: Utc::now(),
        };
        self.emit(
            Some(&pending.question.session_id),
            Some(action_id),
            "pending.resolved",
            Some(&resolution),
        );
        true
    }

    pub fn dismiss_session(&mut self, session_id: &str) -> bool {
        let removed = self.remove_session(session_id, "session dismissed");
        if removed {
            self.emit(Some(session_id), None, "session.removed", None);
        }
        removed
    }

    // ---------- 进程/空闲清理 ----------

    pub fn remove_exited_sessions(
        &mut self,
        is_exited: impl Fn(&SessionSnapshot) -> bool,
        reason: &str,
    ) -> bool {
        let exited: Vec<String> = self
            .sessions
            .values()
            .filter(|s| is_exited(s))
            .map(|s| s.session_id.clone())
            .collect();
        if exited.is_empty() {
            return false;
        }
        let mut removed_any = false;
        for id in &exited {
            removed_any = self.remove_session(id, reason) || removed_any;
        }
        if removed_any {
            self.emit(Some(&exited[0]), None, "session.removed", None);
        }
        removed_any
    }

    pub fn remove_idle_sessions(
        &mut self,
        idle_timeout: Duration,
        now: DateTime<Utc>,
        reason: &str,
    ) -> bool {
        let cutoff = now - chrono::Duration::from_std(idle_timeout).unwrap_or_default();
        let idle: Vec<String> = self
            .sessions
            .values()
            .filter(|s| s.last_updated_at <= cutoff)
            .map(|s| s.session_id.clone())
            .collect();
        if idle.is_empty() {
            return false;
        }
        for id in &idle {
            self.remove_session(id, reason);
        }
        self.emit(Some(&idle[0]), None, "session.removed", None);
        true
    }

    // ---------- 内部 ----------

    fn apply_event(&mut self, evt: &HookEvent) -> (Option<String>, Option<String>, SideEffect) {
        let resolved = self.resolve_session_id(evt);
        let existing = resolved
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .cloned();
        let (new_state, reduced) = SessionSnapshot::reduce_event(existing.clone(), evt);
        let new_state = apply_transcript_messages(existing.as_ref(), new_state);

        let session_id = new_state.session_id.clone();
        let normalized = normalize_event_name(&new_state.source, &evt.event_name);
        let effect =
            if normalized == "Stop" && reduced.is_none() && has_completion_content(&new_state) {
                SideEffect::PlaySound {
                    sound_name: "complete".to_string(),
                }
            } else {
                reduced
            };

        if normalized == "SessionEnd" {
            self.remove_session(&session_id, "session ended");
        } else {
            self.sessions.insert(session_id.clone(), new_state);
        }

        (Some(session_id), Some(normalized), effect)
    }

    fn resolve_session_id(&self, evt: &HookEvent) -> Option<String> {
        if let Some(sid) = &evt.session_id
            && !sid.trim().is_empty()
        {
            return Some(sid.clone());
        }
        if self.sessions.len() == 1 {
            return self.sessions.keys().next().cloned();
        }
        if let Some(pid) = evt.tracked_pid {
            let matches: Vec<&SessionSnapshot> =
                self.sessions.values().filter(|s| s.pid == pid).collect();
            if matches.len() == 1 {
                return Some(matches[0].session_id.clone());
            }
        }
        None
    }

    fn remove_session(&mut self, session_id: &str, reason: &str) -> bool {
        let removed_session = self.sessions.remove(session_id).is_some();
        let removed_pending = self.remove_pending_for_session(session_id, reason);
        removed_session || removed_pending
    }

    fn remove_pending_for_session(&mut self, session_id: &str, reason: &str) -> bool {
        let mut removed = false;

        let mut retained_perms = VecDeque::new();
        while let Some(p) = self.permission_queue.pop_front() {
            if p.request.session_id == session_id {
                let _ = p
                    .completion
                    .send(hook_response_builder::build_permission_deny_response(
                        &p.event, reason,
                    ));
                removed = true;
            } else {
                retained_perms.push_back(p);
            }
        }
        self.permission_queue = retained_perms;

        let mut retained_questions = VecDeque::new();
        while let Some(q) = self.question_queue.pop_front() {
            if q.question.session_id == session_id {
                let _ = q
                    .completion
                    .send(hook_response_builder::build_question_dismiss_response(
                        &q.event, reason,
                    ));
                removed = true;
            } else {
                retained_questions.push_back(q);
            }
        }
        self.question_queue = retained_questions;

        removed
    }

    fn take_permission(&mut self, action_id: &str) -> Option<PendingPermission> {
        let pos = self
            .permission_queue
            .iter()
            .position(|p| p.action_id == action_id)?;
        self.permission_queue.remove(pos)
    }

    fn take_question(&mut self, action_id: &str) -> Option<PendingQuestion> {
        let pos = self
            .question_queue
            .iter()
            .position(|q| q.action_id == action_id)?;
        self.question_queue.remove(pos)
    }

    fn record_history(&mut self, resolution: PendingResolutionDto) {
        self.history.push_back(resolution);
        while self.history.len() > MAX_HISTORY_ENTRIES {
            self.history.pop_front();
        }
    }

    fn session_source_key(&self, session_id: &str) -> String {
        session_source_key(self.sessions.get(session_id))
    }

    fn map_permission_action(&self, p: &PendingPermission) -> PendingActionDto {
        let session = self.sessions.get(&p.request.session_id);
        PendingActionDto {
            action_id: p.action_id.clone(),
            kind: "permission".to_string(),
            session_id: p.request.session_id.clone(),
            source: session_source_key(session),
            source_display_name: SupportedSource::get_display_name(&session_source_key(session))
                .to_string(),
            project_name: Some(session_project_name(session)),
            working_directory: session.and_then(|s| s.working_directory.clone()),
            created_at_utc: p.created_at,
            permission: Some(map_permission(&p.request)),
            question: None,
        }
    }

    fn map_question_action(&self, q: &PendingQuestion) -> PendingActionDto {
        let session = self.sessions.get(&q.question.session_id);
        PendingActionDto {
            action_id: q.action_id.clone(),
            kind: "question".to_string(),
            session_id: q.question.session_id.clone(),
            source: session_source_key(session),
            source_display_name: SupportedSource::get_display_name(&session_source_key(session))
                .to_string(),
            project_name: Some(session_project_name(session)),
            working_directory: session.and_then(|s| s.working_directory.clone()),
            created_at_utc: q.created_at,
            permission: None,
            question: Some(map_question(
                &q.question,
                q.current_question_index,
                Some(q.current_answer_key()),
            )),
        }
    }

    fn emit(
        &self,
        session_id: Option<&str>,
        action_id: Option<&str>,
        realtime_type: &str,
        resolution: Option<&PendingResolutionDto>,
    ) {
        let data = self.realtime_payload(realtime_type, session_id, action_id, resolution);
        let _ = self.events.send(HubEventDto {
            event_type: realtime_type.to_string(),
            timestamp_utc: Utc::now(),
            data,
        });

        if realtime_type == "pending.updated" && session_id.map(|s| !s.is_empty()).unwrap_or(false)
        {
            let data = self.realtime_payload("session.updated", session_id, action_id, None);
            let _ = self.events.send(HubEventDto {
                event_type: "session.updated".to_string(),
                timestamp_utc: Utc::now(),
                data,
            });
        }
    }

    fn realtime_payload(
        &self,
        realtime_type: &str,
        session_id: Option<&str>,
        action_id: Option<&str>,
        resolution: Option<&PendingResolutionDto>,
    ) -> Option<Value> {
        match realtime_type {
            "session.updated" => serde_json::to_value(self.get_sessions()).ok(),
            "session.removed" => Some(json!({ "sessionId": session_id })),
            "pending.updated" => serde_json::to_value(self.get_pending_actions()).ok(),
            "pending.resolved" => Some(json!({
                "actionId": action_id,
                "resolution": resolution,
                "pending": self.get_pending_actions(),
            })),
            _ => None,
        }
    }
}

impl Default for HubState {
    fn default() -> Self {
        Self::new()
    }
}

/// 阻塞事件完整流程：应用 → 等待用户决策 / 超时
pub async fn handle_blocking_event(
    state: &Arc<RwLock<HubState>>,
    evt: HookEvent,
    timeout: Duration,
) -> String {
    let outcome = state.write().await.begin_blocking_event(evt);
    match outcome {
        BlockingOutcome::Immediate(response) => response,
        BlockingOutcome::Pending(handle) => {
            let handle = *handle;
            match tokio::time::timeout(timeout, handle.rx).await {
                Ok(Ok(response)) => response,
                _ => state.write().await.resolve_timeout(
                    &handle.action_id,
                    handle.session_id.as_deref(),
                    handle.kind,
                    &handle.event,
                ),
            }
        }
    }
}

// ---------- 自由辅助函数 ----------

fn new_action_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

fn build_timeout_response(evt: &HookEvent) -> String {
    let normalized =
        normalize_event_name(evt.source.as_deref().unwrap_or("unknown"), &evt.event_name);
    if is_question_event(evt, &normalized) {
        hook_response_builder::build_question_dismiss_response(evt, "timeout")
    } else {
        hook_response_builder::build_permission_deny_response(evt, "timeout")
    }
}

fn is_question_event(evt: &HookEvent, normalized: &str) -> bool {
    if hook_tool_classifier::should_block_question_tool(evt, normalized) {
        return true;
    }
    if !normalized.starts_with("Question") && normalized != "Notification" {
        return false;
    }
    contains_any(Some(&evt.raw_json), &["question", "questions"])
        || contains_any(evt.tool_input.as_ref(), &["question", "questions"])
}

fn contains_any(element: Option<&Value>, names: &[&str]) -> bool {
    let Some(Value::Object(obj)) = element else {
        return false;
    };
    for (key, value) in obj {
        if names.iter().any(|n| n.eq_ignore_ascii_case(key)) {
            return true;
        }
        if value.is_object() && contains_any(Some(value), names) {
            return true;
        }
    }
    false
}

fn to_realtime_event_type(normalized: Option<&str>, effect: &SideEffect) -> &'static str {
    if normalized == Some("SessionEnd") {
        return "session.removed";
    }
    match effect {
        SideEffect::ShowApprovalCard { .. } | SideEffect::ShowQuestionCard { .. } => {
            "pending.updated"
        }
        _ => "session.updated",
    }
}

fn has_completion_content(session: &SessionSnapshot) -> bool {
    session
        .completion_text
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
        || session
            .last_assistant_message
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
}

fn apply_transcript_messages(
    existing: Option<&SessionSnapshot>,
    snapshot: SessionSnapshot,
) -> SessionSnapshot {
    let Some(path) = snapshot
        .transcript_path
        .clone()
        .filter(|p| !p.trim().is_empty())
    else {
        return snapshot;
    };

    let start = match existing {
        Some(e) if e.transcript_path == snapshot.transcript_path => e.transcript_position,
        _ => 0,
    };
    let result = read_new_messages(&path, start);
    if result.position == start && result.messages.is_empty() {
        return snapshot;
    }

    let mut clone = snapshot;
    clone.transcript_position = result.position;
    for message in result.messages {
        SessionSnapshot::add_recent_message(&mut clone, message);
    }
    if clone.completion_text.is_none() {
        clone.completion_text = clone.last_assistant_message.clone();
    }
    clone
}

fn clean_answers(answers: &[String]) -> Vec<String> {
    answers
        .iter()
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .map(str::to_string)
        .collect()
}

fn set_answer(answers: &mut Vec<(String, Vec<String>)>, key: &str, values: Vec<String>) {
    if let Some(entry) = answers.iter_mut().find(|(k, _)| k == key) {
        entry.1 = values;
    } else {
        answers.push((key.to_string(), values));
    }
}

fn advance_question_if_needed(pending: &mut PendingQuestion) -> bool {
    let Some(questions) = pending.question.questions.as_ref() else {
        return false;
    };
    if questions.is_empty() || pending.current_question_index >= questions.len() as i32 - 1 {
        return false;
    }
    pending.current_question_index += 1;
    true
}

fn session_source_key(session: Option<&SessionSnapshot>) -> String {
    match session {
        Some(s) if !s.source.trim().is_empty() => s.source.clone(),
        _ => "unknown".to_string(),
    }
}

fn session_project_name(session: Option<&SessionSnapshot>) -> String {
    let Some(s) = session else {
        return "unknown".to_string();
    };
    s.project_name
        .clone()
        .or_else(|| s.working_directory.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

// ---------- DTO 映射 ----------

fn map_session(session: &SessionSnapshot) -> SessionDto {
    SessionDto {
        session_id: session.session_id.clone(),
        source: session.source.clone(),
        source_display_name: SupportedSource::get_display_name(&session.source).to_string(),
        project_name: session.project_name.clone(),
        working_directory: session.working_directory.clone(),
        status: session.status.to_string(),
        current_tool_name: session.current_tool_name.clone(),
        current_tool_description: session.current_tool_description.clone(),
        created_at_utc: session.created_at,
        last_updated_at_utc: session.last_updated_at,
        tracked_pid: if session.pid == 0 {
            None
        } else {
            Some(session.pid)
        },
        tracked_process_started_at_utc: session.tracked_process_started_at_utc,
        last_user_prompt: session.last_user_prompt.clone(),
        last_assistant_message: session.last_assistant_message.clone(),
        completion_text: session.completion_text.clone(),
        transcript_path: session.transcript_path.clone(),
        transcript_position: session.transcript_position,
        terminal_app: session.terminal_app.clone(),
        terminal_session_id: session.terminal_session_id.clone(),
        recent_messages: session.recent_messages.iter().map(map_message).collect(),
        tool_history: session.tool_history.iter().map(map_tool_history).collect(),
    }
}

fn map_message(message: &ChatMessage) -> ChatMessageDto {
    ChatMessageDto {
        is_user: message.is_user,
        text: message.text.clone(),
        timestamp_utc: message.timestamp,
    }
}

fn map_tool_history(entry: &ToolHistoryEntry) -> ToolHistoryEntryDto {
    ToolHistoryEntryDto {
        tool_name: entry.tool_name.clone(),
        timestamp_utc: entry.timestamp,
        description: entry.description.clone(),
        success: entry.success,
    }
}

fn map_permission(request: &PermissionRequest) -> PermissionRequestDto {
    PermissionRequestDto {
        session_id: request.session_id.clone(),
        tool_name: request.tool_name.clone(),
        tool_use_id: request.tool_use_id.clone(),
        tool_input: request.tool_input.clone(),
        description: request.description.clone(),
        hook_event_name: request.hook_event_name.clone(),
    }
}

fn map_question(
    question: &QuestionData,
    current_question_index: i32,
    current_answer_key: Option<String>,
) -> QuestionDto {
    QuestionDto {
        session_id: question.session_id.clone(),
        id: question.id.clone(),
        question: question.question.clone(),
        header: question.header.clone(),
        options: question
            .options
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(map_question_option)
            .collect(),
        multi_select: question.multi_select,
        is_multi_question: question.is_multi_question,
        questions: question
            .questions
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(map_question_item)
            .collect(),
        hook_event_name: question.hook_event_name.clone(),
        is_ask_user_question: question.is_ask_user_question,
        is_codex_request_user_input: question.is_codex_request_user_input,
        current_question_index,
        current_answer_key: current_answer_key
            .or_else(|| question.id.clone())
            .unwrap_or_else(|| question.question.clone()),
    }
}

fn map_question_item(item: &QuestionItem) -> QuestionItemDto {
    QuestionItemDto {
        id: item.id.clone(),
        question: item.question.clone(),
        header: item.header.clone(),
        options: item
            .options
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(map_question_option)
            .collect(),
        multi_select: item.multi_select,
        allow_free_text: item.allow_free_text,
    }
}

fn map_question_option(option: &QuestionOption) -> QuestionOptionDto {
    QuestionOptionDto {
        label: option.label.clone(),
        description: option.description.clone(),
        value: option.value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn permission_event(session_id: &str, tool_name: &str) -> HookEvent {
        HookEvent {
            event_name: "PermissionRequest".to_string(),
            session_id: Some(session_id.to_string()),
            tool_name: Some(tool_name.to_string()),
            tool_use_id: None,
            agent_id: None,
            tool_input: None,
            raw_json: json!({
                "hook_event_name": "PermissionRequest",
                "session_id": session_id,
                "tool_name": tool_name,
            }),
            source: Some("claude".to_string()),
            parent_pid: None,
            tracked_pid: None,
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        }
    }

    fn session_start(session_id: &str) -> HookEvent {
        HookEvent {
            event_name: "SessionStart".to_string(),
            session_id: Some(session_id.to_string()),
            tool_name: None,
            tool_use_id: None,
            agent_id: None,
            tool_input: None,
            raw_json: json!({ "hook_event_name": "SessionStart" }),
            source: Some("claude".to_string()),
            parent_pid: None,
            tracked_pid: None,
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        }
    }

    fn pending_action_id(outcome: &BlockingOutcome) -> String {
        match outcome {
            BlockingOutcome::Pending(h) => h.action_id.clone(),
            BlockingOutcome::Immediate(_) => panic!("expected pending"),
        }
    }

    #[test]
    fn handle_event_creates_session() {
        let mut state = HubState::new();
        state.handle_event(&session_start("s1"));
        assert_eq!(state.get_sessions().len(), 1);
        assert_eq!(state.get_session("s1").unwrap().session_id, "s1");
    }

    #[test]
    fn blocking_permission_enqueues_pending() {
        let mut state = HubState::new();
        let outcome = state.begin_blocking_event(permission_event("s1", "Bash"));
        assert!(matches!(outcome, BlockingOutcome::Pending(_)));
        assert_eq!(state.get_pending_actions().len(), 1);
    }

    #[tokio::test]
    async fn permission_allow_resolves_blocking_call() {
        let state = Arc::new(RwLock::new(HubState::new()));
        let st = state.clone();
        let task = tokio::spawn(async move {
            handle_blocking_event(&st, permission_event("s1", "Bash"), Duration::from_secs(5)).await
        });

        // 等待 pending 出现
        let action_id = loop {
            let pending = state.read().await.get_pending_actions();
            if let Some(p) = pending.first() {
                break p.action_id.clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        };

        assert!(
            state
                .write()
                .await
                .allow_permission(&action_id, false, None)
        );
        let response = task.await.unwrap();
        assert!(response.contains("allow"), "response: {response}");
        assert_eq!(state.read().await.get_pending_actions().len(), 0);
        assert_eq!(state.read().await.get_pending_history(10).len(), 1);
    }

    #[tokio::test]
    async fn blocking_call_times_out() {
        let state = Arc::new(RwLock::new(HubState::new()));
        let response = handle_blocking_event(
            &state,
            permission_event("s1", "Bash"),
            Duration::from_millis(40),
        )
        .await;
        assert!(
            response.contains("deny") || response.contains("timeout"),
            "response: {response}"
        );
        assert_eq!(state.read().await.get_pending_actions().len(), 0);
    }

    #[test]
    fn auto_approve_skips_pending() {
        let mut state = HubState::with_auto_approve(Some(Box::new(|req| req.tool_name == "Read")));
        let outcome = state.begin_blocking_event(permission_event("s1", "Read"));
        assert!(matches!(outcome, BlockingOutcome::Immediate(_)));
        assert_eq!(state.get_pending_actions().len(), 0);
    }

    #[test]
    fn history_capped_at_200() {
        let mut state = HubState::new();
        for i in 0..205 {
            let outcome = state.begin_blocking_event(permission_event("s1", "Bash"));
            let action_id = pending_action_id(&outcome);
            assert!(state.allow_permission(&action_id, false, None));
            let _ = i;
        }
        assert_eq!(state.get_pending_history(1000).len(), MAX_HISTORY_ENTRIES);
    }

    #[test]
    fn session_end_cascades_pending_cleanup() {
        let mut state = HubState::new();
        let _outcome = state.begin_blocking_event(permission_event("s1", "Bash"));
        assert_eq!(state.get_pending_actions().len(), 1);

        // SessionEnd 应移除会话并级联清理待处理操作
        state.handle_event(&HookEvent {
            event_name: "SessionEnd".to_string(),
            session_id: Some("s1".to_string()),
            tool_name: None,
            tool_use_id: None,
            agent_id: None,
            tool_input: None,
            raw_json: json!({ "hook_event_name": "SessionEnd" }),
            source: Some("claude".to_string()),
            parent_pid: None,
            tracked_pid: None,
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        });

        assert_eq!(state.get_pending_actions().len(), 0);
        assert!(state.get_session("s1").is_none());
    }

    #[test]
    fn deny_permission_resolves() {
        let mut state = HubState::new();
        let outcome = state.begin_blocking_event(permission_event("s1", "Bash"));
        let action_id = pending_action_id(&outcome);
        assert!(state.deny_permission(&action_id, "nope", Some("tester".into())));
        assert_eq!(state.get_pending_actions().len(), 0);
        let history = state.get_pending_history(10);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].decision, "deny");
    }

    #[test]
    fn idle_sessions_removed() {
        let mut state = HubState::new();
        state.handle_event(&session_start("s1"));
        assert_eq!(state.get_sessions().len(), 1);

        // 用未来时间点触发空闲清理
        let future = Utc::now() + chrono::Duration::hours(1);
        let removed = state.remove_idle_sessions(Duration::from_secs(60), future, "idle");
        assert!(removed);
        assert_eq!(state.get_sessions().len(), 0);
    }

    #[tokio::test]
    async fn publish_broadcasts_to_subscriber() {
        let state = HubState::new();
        let mut rx = state.subscribe();
        state.publish("terminal.activate", Some(json!({ "sessionId": "s1" })));
        let evt = rx.recv().await.unwrap();
        assert_eq!(evt.event_type, "terminal.activate");
        assert_eq!(evt.data.unwrap()["sessionId"], "s1");
    }
}
