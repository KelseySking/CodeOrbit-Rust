//! HubState — 中央状态机：会话、待处理操作（权限/问题）生命周期与实时事件广播

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
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

/// 单个阻塞 hook 连接的等待者（合流后同一 pending 可有多个）
struct PendingWaiter {
    event: HookEvent,
    completion: oneshot::Sender<String>,
}

/// 队列中的待处理权限请求（multi-waiter：Claude PreToolUse + PermissionRequest 合流）
struct PendingPermission {
    action_id: String,
    created_at: DateTime<Utc>,
    request: PermissionRequest,
    waiters: Vec<PendingWaiter>,
    dedupe_key: Option<String>,
}

/// 队列中的待处理问题（multi-waiter）
struct PendingQuestion {
    action_id: String,
    created_at: DateTime<Utc>,
    question: QuestionData,
    waiters: Vec<PendingWaiter>,
    dedupe_key: Option<String>,
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

    /// 阻塞事件第一阶段：应用事件、创建待处理操作（如需要）。
    ///
    /// Claude 同一工具调用可能同时触发 `PreToolUse` 与 `PermissionRequest`（或
    /// AskUserQuestion 双事件）。此处按 dedupe key 合流为单条 pending，
    /// multi-waiter fan-out 解析。
    pub fn begin_blocking_event(&mut self, evt: HookEvent) -> BlockingOutcome {
        let (session_id, normalized, effect) = self.apply_event(&evt);
        let rt = to_realtime_event_type(normalized.as_deref(), &effect);

        let outcome = match &effect {
            SideEffect::ShowApprovalCard { request, .. } => {
                self.begin_permission_blocking(evt.clone(), request.clone())
            }
            SideEffect::ShowQuestionCard { question, .. } => {
                self.begin_question_blocking(evt.clone(), question.clone())
            }
            // 无卡片效果：等同于超时拒绝/关闭（mirror C# 行为，但即时返回）
            _ => BlockingOutcome::Immediate(build_timeout_response(&evt)),
        };

        self.emit(session_id.as_deref(), None, rt, None);
        outcome
    }

    fn begin_permission_blocking(
        &mut self,
        evt: HookEvent,
        request: PermissionRequest,
    ) -> BlockingOutcome {
        let dedupe_key = permission_dedupe_key(&request, &evt);

        if let Some(key) = dedupe_key.as_ref()
            && let Some(pos) = self.find_permission_by_key(key)
        {
            let (tx, rx) = oneshot::channel();
            let pending = &mut self.permission_queue[pos];
            // 后到事件可回填展示字段
            if pending.request.tool_use_id.is_none() {
                if let Some(id) = request
                    .tool_use_id
                    .clone()
                    .or_else(|| evt.tool_use_id.clone())
                {
                    pending.request.tool_use_id = Some(id);
                }
            }
            if pending.request.description.is_none() && request.description.is_some() {
                pending.request.description = request.description.clone();
            }
            if pending.request.tool_input.is_none() && request.tool_input.is_some() {
                pending.request.tool_input = request.tool_input.clone();
            }
            let action_id = pending.action_id.clone();
            let sid = pending.request.session_id.clone();
            pending.waiters.push(PendingWaiter {
                event: evt.clone(),
                completion: tx,
            });
            return BlockingOutcome::Pending(Box::new(PendingHandle {
                action_id,
                session_id: Some(sid),
                kind: "permission",
                event: evt,
                rx,
            }));
        }

        let auto = self
            .should_auto_approve
            .as_ref()
            .map(|f| f(&request))
            .unwrap_or(false);
        if auto {
            return BlockingOutcome::Immediate(
                hook_response_builder::build_permission_allow_response(&evt, Some(&request), false),
            );
        }

        let (tx, rx) = oneshot::channel();
        let action_id = new_action_id("permission");
        let sid = request.session_id.clone();
        self.permission_queue.push_back(PendingPermission {
            action_id: action_id.clone(),
            created_at: Utc::now(),
            request,
            waiters: vec![PendingWaiter {
                event: evt.clone(),
                completion: tx,
            }],
            dedupe_key,
        });
        BlockingOutcome::Pending(Box::new(PendingHandle {
            action_id,
            session_id: Some(sid),
            kind: "permission",
            event: evt,
            rx,
        }))
    }

    fn begin_question_blocking(
        &mut self,
        evt: HookEvent,
        question: QuestionData,
    ) -> BlockingOutcome {
        let dedupe_key = question_dedupe_key(&question, &evt);

        if let Some(key) = dedupe_key.as_ref()
            && let Some(pos) = self.find_question_by_key(key)
        {
            let (tx, rx) = oneshot::channel();
            let pending = &mut self.question_queue[pos];
            if pending.question.id.is_none() && question.id.is_some() {
                pending.question.id = question.id.clone();
            }
            if pending.question.question.is_empty() && !question.question.is_empty() {
                pending.question.question = question.question.clone();
            }
            if pending.question.original_input.is_none() && question.original_input.is_some() {
                pending.question.original_input = question.original_input.clone();
            }
            let action_id = pending.action_id.clone();
            let sid = pending.question.session_id.clone();
            pending.waiters.push(PendingWaiter {
                event: evt.clone(),
                completion: tx,
            });
            return BlockingOutcome::Pending(Box::new(PendingHandle {
                action_id,
                session_id: Some(sid),
                kind: "question",
                event: evt,
                rx,
            }));
        }

        let (tx, rx) = oneshot::channel();
        let action_id = new_action_id("question");
        let sid = question.session_id.clone();
        self.question_queue.push_back(PendingQuestion {
            action_id: action_id.clone(),
            created_at: Utc::now(),
            question,
            waiters: vec![PendingWaiter {
                event: evt.clone(),
                completion: tx,
            }],
            dedupe_key,
            current_question_index: 0,
            answers: Vec::new(),
        });
        BlockingOutcome::Pending(Box::new(PendingHandle {
            action_id,
            session_id: Some(sid),
            kind: "question",
            event: evt,
            rx,
        }))
    }

    /// 阻塞超时处理：take 幂等；首个 timeout fan-out 所有 waiters，后续只返回响应字符串
    pub fn resolve_timeout(
        &mut self,
        action_id: &str,
        session_id: Option<&str>,
        kind: &str,
        evt: &HookEvent,
    ) -> String {
        match kind {
            "permission" => {
                if let Some(pending) = self.take_permission(action_id) {
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
                    for waiter in pending.waiters {
                        let response = build_timeout_response(&waiter.event);
                        let _ = waiter.completion.send(response);
                    }
                    self.emit(
                        session_id,
                        Some(action_id),
                        "pending.resolved",
                        Some(&resolution),
                    );
                } else {
                    self.emit(session_id, None, "pending.updated", None);
                }
            }
            "question" => {
                if let Some(pending) = self.take_question(action_id) {
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
                    for waiter in pending.waiters {
                        let response = build_timeout_response(&waiter.event);
                        let _ = waiter.completion.send(response);
                    }
                    self.emit(
                        session_id,
                        Some(action_id),
                        "pending.resolved",
                        Some(&resolution),
                    );
                } else {
                    self.emit(session_id, None, "pending.updated", None);
                }
            }
            _ => {
                self.emit(session_id, None, "pending.updated", None);
            }
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

        for waiter in pending.waiters {
            let response = hook_response_builder::build_permission_allow_response(
                &waiter.event,
                Some(&pending.request),
                always,
            );
            let _ = waiter.completion.send(response);
        }
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

        for waiter in pending.waiters {
            let response =
                hook_response_builder::build_permission_deny_response(&waiter.event, reason);
            let _ = waiter.completion.send(response);
        }
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
        self.record_history(resolution.clone());
        for waiter in pending.waiters {
            let response = hook_response_builder::build_question_answer_response(
                &waiter.event,
                &pending.question,
                &pending.answers,
            );
            let _ = waiter.completion.send(response);
        }
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

        for waiter in pending.waiters {
            let response = hook_response_builder::build_question_answer_response(
                &waiter.event,
                &pending.question,
                &pending.answers,
            );
            let _ = waiter.completion.send(response);
        }
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
        self.record_history(resolution.clone());
        for waiter in pending.waiters {
            let response =
                hook_response_builder::build_question_dismiss_response(&waiter.event, reason);
            let _ = waiter.completion.send(response);
        }
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
        let (mut new_state, reduced) = SessionSnapshot::reduce_event(existing.clone(), evt);
        if let Some(resolved) = &resolved {
            new_state.session_id = resolved.clone();
        }
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
        if let Some(pid) = evt.tracked_pid {
            let matches: Vec<&SessionSnapshot> = self
                .sessions
                .values()
                .filter(|s| same_tracked_process(s, pid, evt.tracked_process_started_at_utc))
                .collect();
            if matches.len() == 1 {
                return Some(matches[0].session_id.clone());
            }
        }
        if let Some(sid) = synthetic_session_id(evt) {
            return Some(sid);
        }
        if self.sessions.len() == 1 {
            return self.sessions.keys().next().cloned();
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
                for waiter in p.waiters {
                    let _ = waiter.completion.send(
                        hook_response_builder::build_permission_deny_response(
                            &waiter.event,
                            reason,
                        ),
                    );
                }
                removed = true;
            } else {
                retained_perms.push_back(p);
            }
        }
        self.permission_queue = retained_perms;

        let mut retained_questions = VecDeque::new();
        while let Some(q) = self.question_queue.pop_front() {
            if q.question.session_id == session_id {
                for waiter in q.waiters {
                    let _ = waiter.completion.send(
                        hook_response_builder::build_question_dismiss_response(
                            &waiter.event,
                            reason,
                        ),
                    );
                }
                removed = true;
            } else {
                retained_questions.push_back(q);
            }
        }
        self.question_queue = retained_questions;

        removed
    }

    fn find_permission_by_key(&self, key: &str) -> Option<usize> {
        self.permission_queue
            .iter()
            .position(|p| p.dedupe_key.as_deref() == Some(key))
    }

    fn find_question_by_key(&self, key: &str) -> Option<usize> {
        self.question_queue
            .iter()
            .position(|q| q.dedupe_key.as_deref() == Some(key))
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

/// 权限去重键：优先 tool_use_id；否则 tool_name + 稳定 input 指纹。
/// 无 tool_use_id 且无有效 input → None（不合并）。
fn permission_dedupe_key(request: &PermissionRequest, evt: &HookEvent) -> Option<String> {
    let session_id = non_empty(&request.session_id)
        .or_else(|| evt.session_id.as_deref().and_then(non_empty))?;
    let tool_use_id = request
        .tool_use_id
        .as_deref()
        .and_then(non_empty)
        .or_else(|| evt.tool_use_id.as_deref().and_then(non_empty));
    if let Some(tu) = tool_use_id {
        return Some(format!("permission|{session_id}|tu:{tu}"));
    }

    let tool_name = non_empty(&request.tool_name)
        .or_else(|| evt.tool_name.as_deref().and_then(non_empty))?;
    let input_fp = fingerprint_permission_input(request, evt)?;
    Some(format!("permission|{session_id}|fp:{tool_name}|{input_fp}"))
}

/// 问题去重键：优先 tool_use_id；否则 question id / 文本 / tool_input 指纹。
fn question_dedupe_key(question: &QuestionData, evt: &HookEvent) -> Option<String> {
    let session_id = non_empty(&question.session_id)
        .or_else(|| evt.session_id.as_deref().and_then(non_empty))?;
    let tool_use_id = evt.tool_use_id.as_deref().and_then(non_empty);
    if let Some(tu) = tool_use_id {
        return Some(format!("question|{session_id}|tu:{tu}"));
    }

    let tool_name = evt.tool_name.as_deref().and_then(non_empty).unwrap_or("");
    let q_fp = question
        .id
        .as_deref()
        .and_then(non_empty)
        .map(|id| format!("id:{id}"))
        .or_else(|| {
            let text = non_empty(&question.question)?;
            let count = question
                .questions
                .as_ref()
                .map(|q| q.len())
                .unwrap_or(0);
            Some(format!("qt:{text}|n:{count}"))
        })
        .or_else(|| {
            let input = evt.tool_input.as_ref().or(question.original_input.as_ref())?;
            if is_empty_json(input) {
                return None;
            }
            Some(format!("in:{}", stable_json_fingerprint(input)))
        })?;
    Some(format!("question|{session_id}|fp:{tool_name}|{q_fp}"))
}

fn fingerprint_permission_input(
    request: &PermissionRequest,
    evt: &HookEvent,
) -> Option<String> {
    if let Some(map) = &request.tool_input
        && !map.is_empty()
    {
        let value = Value::Object(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        return Some(stable_json_fingerprint(&value));
    }
    let input = evt.tool_input.as_ref()?;
    if is_empty_json(input) {
        return None;
    }
    Some(stable_json_fingerprint(input))
}

/// 对 JSON 做键排序后的稳定指纹（进程内 DefaultHasher，不要求跨进程）。
fn stable_json_fingerprint(value: &Value) -> String {
    let mut hasher = DefaultHasher::new();
    hash_stable_json(value, &mut hasher);
    format!("{:x}", hasher.finish())
}

fn hash_stable_json(value: &Value, hasher: &mut DefaultHasher) {
    match value {
        Value::Null => 0u8.hash(hasher),
        Value::Bool(b) => {
            1u8.hash(hasher);
            b.hash(hasher);
        }
        Value::Number(n) => {
            2u8.hash(hasher);
            n.to_string().hash(hasher);
        }
        Value::String(s) => {
            3u8.hash(hasher);
            s.hash(hasher);
        }
        Value::Array(items) => {
            4u8.hash(hasher);
            items.len().hash(hasher);
            for item in items {
                hash_stable_json(item, hasher);
            }
        }
        Value::Object(map) => {
            5u8.hash(hasher);
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            keys.len().hash(hasher);
            for key in keys {
                key.hash(hasher);
                hash_stable_json(&map[key], hasher);
            }
        }
    }
}

fn is_empty_json(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Object(m) => m.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t) }
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

fn same_tracked_process(
    session: &SessionSnapshot,
    pid: u32,
    started_at: Option<DateTime<Utc>>,
) -> bool {
    if session.pid != pid {
        return false;
    }
    match (session.tracked_process_started_at_utc, started_at) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

fn synthetic_session_id(evt: &HookEvent) -> Option<String> {
    let pid = evt.tracked_pid?;
    let source = evt
        .source
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("unknown");
    let started = evt
        .tracked_process_started_at_utc
        .map(|dt| dt.timestamp().to_string())
        .unwrap_or_else(|| "unknown-start".to_string());
    Some(format!("process-{source}-{pid}-{started}"))
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

    fn session_start_without_id(pid: u32, started_at: &str) -> HookEvent {
        HookEvent {
            event_name: "SessionStart".to_string(),
            session_id: None,
            tool_name: None,
            tool_use_id: None,
            agent_id: None,
            tool_input: None,
            raw_json: json!({ "hook_event_name": "SessionStart" }),
            source: Some("claude".to_string()),
            parent_pid: None,
            tracked_pid: Some(pid),
            tracked_pid_kind: Some("shell".to_string()),
            tracked_process_started_at_utc: Some(started_at.parse::<DateTime<Utc>>().unwrap()),
        }
    }

    fn codex_request_user_input_event() -> HookEvent {
        HookEvent {
            event_name: "PreToolUse".to_string(),
            session_id: Some("s1".to_string()),
            tool_name: Some("functions.request_user_input".to_string()),
            tool_use_id: None,
            agent_id: None,
            tool_input: Some(json!({
                "questions": [
                    { "id": "next", "question": "Next step?" }
                ]
            })),
            raw_json: json!({
                "hook_event_name": "PreToolUse",
                "session_id": "s1",
                "tool_name": "functions.request_user_input",
                "tool_input": {
                    "questions": [
                        { "id": "next", "question": "Next step?" }
                    ]
                }
            }),
            source: Some("codex".to_string()),
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
    fn missing_session_id_uses_stable_process_key() {
        let mut state = HubState::new();
        let event = session_start_without_id(42, "2026-07-03T00:00:00Z");

        state.handle_event(&event);
        state.handle_event(&event);

        let sessions = state.get_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "process-claude-42-1783036800");
    }

    #[test]
    fn process_key_distinguishes_reused_pid_start_time() {
        let mut state = HubState::new();

        state.handle_event(&session_start_without_id(42, "2026-07-03T00:00:00Z"));
        state.handle_event(&session_start_without_id(42, "2026-07-03T00:01:00Z"));

        assert_eq!(state.get_sessions().len(), 2);
    }

    #[test]
    fn blocking_permission_enqueues_pending() {
        let mut state = HubState::new();
        let outcome = state.begin_blocking_event(permission_event("s1", "Bash"));
        assert!(matches!(outcome, BlockingOutcome::Pending(_)));
        assert_eq!(state.get_pending_actions().len(), 1);
    }

    #[test]
    fn codex_request_user_input_enqueues_pending_question() {
        let mut state = HubState::new();
        let outcome = state.begin_blocking_event(codex_request_user_input_event());

        assert!(matches!(outcome, BlockingOutcome::Pending(_)));
        let pending = state.get_pending_actions();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, "question");
        let question = pending[0].question.as_ref().unwrap();
        assert!(question.is_codex_request_user_input);
        assert_eq!(question.questions[0].id.as_deref(), Some("next"));
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

    // ---------- Claude dual-hook pending dedupe ----------

    fn perm_evt(
        name: &str,
        session: &str,
        tool: &str,
        tool_use_id: Option<&str>,
        command: &str,
    ) -> HookEvent {
        let mut raw = json!({
            "hook_event_name": name,
            "session_id": session,
            "tool_name": tool,
            "_source": "claude",
        });
        if name == "PreToolUse" {
            raw["requires_approval"] = json!(true);
        }
        let tool_input = json!({"command": command});
        raw["tool_input"] = tool_input.clone();
        if let Some(id) = tool_use_id {
            raw["tool_use_id"] = json!(id);
        }
        HookEvent {
            event_name: name.into(),
            session_id: Some(session.into()),
            tool_name: Some(tool.into()),
            tool_use_id: tool_use_id.map(str::to_string),
            agent_id: None,
            tool_input: Some(tool_input),
            raw_json: raw,
            source: Some("claude".into()),
            parent_pid: None,
            tracked_pid: None,
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        }
    }

    fn ask_user_question_evt(name: &str, session: &str, tool_use_id: Option<&str>) -> HookEvent {
        let tool_input = json!({
            "questions": [{
                "question": "Pick?",
                "header": "h",
                "options": [{"label": "A"}]
            }]
        });
        let mut raw = json!({
            "hook_event_name": name,
            "session_id": session,
            "tool_name": "AskUserQuestion",
            "tool_input": tool_input,
            "_source": "claude",
        });
        if let Some(id) = tool_use_id {
            raw["tool_use_id"] = json!(id);
        }
        HookEvent {
            event_name: name.into(),
            session_id: Some(session.into()),
            tool_name: Some("AskUserQuestion".into()),
            tool_use_id: tool_use_id.map(str::to_string),
            agent_id: None,
            tool_input: Some(tool_input),
            raw_json: raw,
            source: Some("claude".into()),
            parent_pid: None,
            tracked_pid: None,
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        }
    }

    fn take_pending_rx(outcome: BlockingOutcome) -> (String, oneshot::Receiver<String>) {
        match outcome {
            BlockingOutcome::Pending(h) => {
                let h = *h;
                (h.action_id, h.rx)
            }
            BlockingOutcome::Immediate(r) => panic!("expected pending, got immediate: {r}"),
        }
    }

    #[test]
    fn dedupe_permission_by_tool_use_id() {
        let mut state = HubState::new();
        let pre = perm_evt("PreToolUse", "s1", "Bash", Some("tu-1"), "ls");
        let pr = perm_evt("PermissionRequest", "s1", "Bash", Some("tu-1"), "ls");

        let (id1, mut rx1) = take_pending_rx(state.begin_blocking_event(pre));
        let (id2, mut rx2) = take_pending_rx(state.begin_blocking_event(pr));

        assert_eq!(id1, id2);
        assert_eq!(state.get_pending_actions().len(), 1);
        assert_eq!(state.permission_queue[0].waiters.len(), 2);

        assert!(state.allow_permission(&id1, false, None));
        assert_eq!(state.get_pending_actions().len(), 0);

        let r1 = rx1.try_recv().expect("pre response");
        let r2 = rx2.try_recv().expect("permission response");
        let v1: Value = serde_json::from_str(&r1).unwrap();
        let v2: Value = serde_json::from_str(&r2).unwrap();
        assert_eq!(v1["hookSpecificOutput"]["permissionDecision"], "allow");
        assert_eq!(v1["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(v2["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert_eq!(v2["hookSpecificOutput"]["hookEventName"], "PermissionRequest");
    }

    #[test]
    fn dedupe_permission_by_input_fingerprint() {
        let mut state = HubState::new();
        let pre = perm_evt("PreToolUse", "s1", "Bash", None, "echo hi");
        let pr = perm_evt("PermissionRequest", "s1", "Bash", None, "echo hi");

        let (id1, _) = take_pending_rx(state.begin_blocking_event(pre));
        let (id2, _) = take_pending_rx(state.begin_blocking_event(pr));
        assert_eq!(id1, id2);
        assert_eq!(state.get_pending_actions().len(), 1);
    }

    #[test]
    fn no_merge_different_tool_use_id() {
        let mut state = HubState::new();
        let a = perm_evt("PreToolUse", "s1", "Bash", Some("tu-a"), "ls");
        let b = perm_evt("PermissionRequest", "s1", "Bash", Some("tu-b"), "ls");

        let (id1, _) = take_pending_rx(state.begin_blocking_event(a));
        let (id2, _) = take_pending_rx(state.begin_blocking_event(b));
        assert_ne!(id1, id2);
        assert_eq!(state.get_pending_actions().len(), 2);
    }

    #[test]
    fn no_merge_empty_fingerprint() {
        // 无 tool_use_id 且无 tool_input → 不合并
        let mut state = HubState::new();
        let a = permission_event("s1", "Bash");
        let b = permission_event("s1", "Bash");
        let (id1, _) = take_pending_rx(state.begin_blocking_event(a));
        let (id2, _) = take_pending_rx(state.begin_blocking_event(b));
        assert_ne!(id1, id2);
        assert_eq!(state.get_pending_actions().len(), 2);
    }

    #[test]
    fn deny_and_timeout_fanout() {
        let mut state = HubState::new();
        let pre = perm_evt("PreToolUse", "s1", "Bash", Some("tu-d"), "rm -rf /");
        let pr = perm_evt("PermissionRequest", "s1", "Bash", Some("tu-d"), "rm -rf /");
        let (id, mut rx1) = take_pending_rx(state.begin_blocking_event(pre));
        let (_, mut rx2) = take_pending_rx(state.begin_blocking_event(pr));

        assert!(state.deny_permission(&id, "blocked", None));
        let r1 = rx1.try_recv().unwrap();
        let r2 = rx2.try_recv().unwrap();
        assert!(r1.contains("permissionDecision") || r1.contains("deny"));
        assert!(r2.contains("decision") || r2.contains("deny"));
        let v1: Value = serde_json::from_str(&r1).unwrap();
        let v2: Value = serde_json::from_str(&r2).unwrap();
        assert_eq!(v1["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(v2["hookSpecificOutput"]["decision"]["behavior"], "deny");

        // timeout fan-out on a fresh dual pending
        let mut state = HubState::new();
        let pre = perm_evt("PreToolUse", "s1", "Bash", Some("tu-t"), "sleep");
        let pr = perm_evt("PermissionRequest", "s1", "Bash", Some("tu-t"), "sleep");
        let pre_clone = pre.clone();
        let (id, mut rx1) = take_pending_rx(state.begin_blocking_event(pre));
        let (_, mut rx2) = take_pending_rx(state.begin_blocking_event(pr));
        let resp = state.resolve_timeout(&id, Some("s1"), "permission", &pre_clone);
        assert!(resp.contains("deny") || resp.contains("timeout"));
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
        // 幂等：第二次 timeout 不 panic
        let _ = state.resolve_timeout(&id, Some("s1"), "permission", &pre_clone);
        assert_eq!(state.get_pending_history(10).len(), 1);
    }

    #[test]
    fn ask_user_question_dedupe() {
        let mut state = HubState::new();
        let pre = ask_user_question_evt("PreToolUse", "s1", Some("q-1"));
        let pr = ask_user_question_evt("PermissionRequest", "s1", Some("q-1"));

        let (id1, mut rx1) = take_pending_rx(state.begin_blocking_event(pre));
        let (id2, mut rx2) = take_pending_rx(state.begin_blocking_event(pr));
        assert_eq!(id1, id2);
        assert_eq!(state.get_pending_actions().len(), 1);
        assert_eq!(state.get_pending_actions()[0].kind, "question");

        let answered = state.answer_current_question(&id1, vec!["A".into()], None);
        assert_eq!(answered, (true, true));
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
        assert_eq!(state.get_pending_actions().len(), 0);
        let history = state.get_pending_history(10);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].decision, "answered");
    }

    #[test]
    fn single_event_regression() {
        let mut state = HubState::new();
        let outcome = state.begin_blocking_event(permission_event("s1", "Bash"));
        let (id, mut rx) = take_pending_rx(outcome);
        assert_eq!(state.get_pending_actions().len(), 1);
        assert!(state.allow_permission(&id, false, None));
        let r = rx.try_recv().unwrap();
        let v: Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["hookSpecificOutput"]["decision"]["behavior"], "allow");
    }

    #[test]
    fn auto_approve_first_then_second_new() {
        let mut state = HubState::with_auto_approve(Some(Box::new(|req| req.tool_name == "Bash")));
        // 首条 Immediate allow（无 pending 可挂接）
        let o1 = state.begin_blocking_event(perm_evt(
            "PreToolUse",
            "s1",
            "Bash",
            Some("auto-1"),
            "echo",
        ));
        assert!(matches!(o1, BlockingOutcome::Immediate(_)));
        assert_eq!(state.get_pending_actions().len(), 0);

        // 无 auto 时再来一条应新建
        let mut state = HubState::new();
        let o = state.begin_blocking_event(perm_evt(
            "PermissionRequest",
            "s1",
            "Bash",
            Some("auto-2"),
            "echo",
        ));
        assert!(matches!(o, BlockingOutcome::Pending(_)));
    }

    #[test]
    fn pending_exists_second_attaches_not_auto() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        // 第一次 auto=false 建卡；第二次若误走 auto 路径会 Immediate（auto=true）
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_cb = calls.clone();
        let mut state = HubState::with_auto_approve(Some(Box::new(move |_| {
            // 第 0 次调用返回 false；之后 true
            calls_cb.fetch_add(1, Ordering::SeqCst) > 0
        })));

        let pre = perm_evt("PreToolUse", "s1", "Bash", Some("no-ghost"), "y");
        let (id1, mut rx1) = take_pending_rx(state.begin_blocking_event(pre));
        assert_eq!(state.get_pending_actions().len(), 1);

        let pr = perm_evt("PermissionRequest", "s1", "Bash", Some("no-ghost"), "y");
        let outcome = state.begin_blocking_event(pr);
        match outcome {
            BlockingOutcome::Pending(mut h) => {
                assert_eq!(h.action_id, id1);
                // attach 路径不应再次调用 auto（仍为 1 次）
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                assert!(state.allow_permission(&id1, false, None));
                let _ = rx1.try_recv();
                let _ = h.rx.try_recv();
            }
            BlockingOutcome::Immediate(_) => panic!("should attach, not auto"),
        }
        assert_eq!(state.get_pending_actions().len(), 0);

        // 首条 auto Immediate 后不留幽灵 pending
        let mut state = HubState::with_auto_approve(Some(Box::new(|_| true)));
        let o = state.begin_blocking_event(perm_evt(
            "PreToolUse",
            "s1",
            "Bash",
            Some("ghost"),
            "x",
        ));
        assert!(matches!(o, BlockingOutcome::Immediate(_)));
        assert_eq!(state.get_pending_actions().len(), 0);
        let o2 = state.begin_blocking_event(perm_evt(
            "PermissionRequest",
            "s1",
            "Bash",
            Some("ghost"),
            "x",
        ));
        assert!(matches!(o2, BlockingOutcome::Immediate(_)));
        assert_eq!(state.get_pending_actions().len(), 0);
    }
}
