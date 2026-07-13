//! 核心服务层 — 事件标准化、Hook 响应构建、会话持久化、设置、Transcript 读取等

pub mod codex_home;
pub mod codex_permission_rules;
pub mod event_logger;
pub mod event_normalizer;
pub mod hook_response_builder;
pub mod hook_response_diagnostics;
pub mod hook_tool_classifier;
pub mod l10n;
pub mod session_persistence;
pub mod settings_manager;
pub mod transcript_message_reader;
pub mod transcript_path_resolver;

// 内部响应构建器（经 hook_response_builder 分派）
mod claude_style_hook_response_builder;
mod codex_hook_response_builder;
mod legacy_question_response_builder;

pub use codex_home::resolve_codex_home;
pub use event_logger::{EventLogger, LogKind, log_error};
pub use event_normalizer::{normalize_event_name, normalize_field_name};
pub use hook_tool_classifier::HookQuestionToolKind;
pub use l10n::L10n;
pub use legacy_question_response_builder::Answers;
pub use session_persistence::SessionPersistence;
pub use settings_manager::SettingsManager;
pub use transcript_message_reader::{TranscriptReadResult, read_new_messages};
