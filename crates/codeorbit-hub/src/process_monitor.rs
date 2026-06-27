//! 进程监控 — 每秒扫描，移除跟踪进程已退出的会话（含 PID 复用检测）

use std::sync::Arc;
use std::time::Duration;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::RwLock;

use codeorbit_core::models::SessionSnapshot;

use crate::state::HubState;

/// 进程启动时间允许的误差（秒），用于抵消时钟/采集精度
const START_TIME_TOLERANCE_SECS: u64 = 2;

/// 进程监控器
pub struct ProcessMonitor;

impl ProcessMonitor {
    /// 持续运行：每秒扫描一次
    pub async fn run(state: Arc<RwLock<HubState>>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            Self::scan_once(&state).await;
        }
    }

    /// 单次扫描，返回是否移除了会话
    pub async fn scan_once(state: &Arc<RwLock<HubState>>) -> bool {
        let pids: Vec<Pid> = {
            let guard = state.read().await;
            guard
                .tracked_pids()
                .into_iter()
                .map(Pid::from_u32)
                .collect()
        };
        if pids.is_empty() {
            return false;
        }

        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::everything(),
        );

        let mut guard = state.write().await;
        guard.remove_exited_sessions(|session| is_exited(&sys, session), "process exited")
    }
}

/// 判断会话跟踪的进程是否已退出（不存在，或 PID 被复用）
fn is_exited(sys: &System, session: &SessionSnapshot) -> bool {
    if session.pid == 0 {
        return false;
    }
    match sys.process(Pid::from_u32(session.pid)) {
        None => true,
        Some(process) => {
            // PID 复用检测：进程启动时间与跟踪记录显著不符
            if let Some(tracked) = session.tracked_process_started_at_utc {
                let proc_start = process.start_time();
                let tracked_secs = tracked.timestamp().max(0) as u64;
                if proc_start.abs_diff(tracked_secs) > START_TIME_TOLERANCE_SECS {
                    return true;
                }
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeorbit_core::models::HookEvent;
    use serde_json::json;

    fn session_start_event(session_id: &str, pid: u32) -> HookEvent {
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
            tracked_pid: Some(pid),
            tracked_pid_kind: None,
            tracked_process_started_at_utc: None,
        }
    }

    #[tokio::test]
    async fn removes_session_with_dead_pid_keeps_live() {
        let state = Arc::new(RwLock::new(HubState::new()));

        // 活进程（当前测试进程）+ 死进程（极大不可能存在的 PID）
        let live_pid = std::process::id();
        state
            .write()
            .await
            .handle_event(&session_start_event("live", live_pid));
        state
            .write()
            .await
            .handle_event(&session_start_event("dead", 4_294_900_000));

        assert_eq!(state.read().await.get_sessions().len(), 2);

        ProcessMonitor::scan_once(&state).await;

        let sessions = state.read().await.get_sessions();
        assert_eq!(sessions.len(), 1, "死进程会话应被移除");
        assert_eq!(sessions[0].session_id, "live");
    }
}
