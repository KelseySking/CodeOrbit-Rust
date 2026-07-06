//! CodeOrbit Hub — 状态管理、REST API 和 WebSocket 服务

pub mod api;
pub mod config_installer;
pub mod hook_server;
pub mod process_monitor;
pub mod source_service;
pub mod state;
pub mod wsl_installer;

pub use api::{AppState, router};
pub use hook_server::HookServer;
pub use process_monitor::ProcessMonitor;
pub use state::{AutoApprove, BlockingOutcome, HubState, PendingHandle, handle_blocking_event};
