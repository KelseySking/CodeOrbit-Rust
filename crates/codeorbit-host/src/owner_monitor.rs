//! Owner 进程监控 — 拥有者进程退出后触发关闭

use std::time::Duration;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::Notify;

/// 每秒轮询 owner 进程，退出后通过 `notify` 触发关闭
pub async fn watch(owner_pid: u32, notify: std::sync::Arc<Notify>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        if !is_process_alive(owner_pid) {
            tracing::info!("owner 进程 {owner_pid} 已退出，触发关闭");
            notify.notify_waiters();
            return;
        }
    }
}

/// 检测进程是否存活
pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let mut sys = System::new();
    let target = Pid::from_u32(pid);
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[target]),
        true,
        ProcessRefreshKind::nothing(),
    );
    sys.process(target).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_is_alive() {
        assert!(is_process_alive(std::process::id()));
    }

    #[test]
    fn bogus_pid_is_not_alive() {
        assert!(!is_process_alive(4_294_900_001));
        assert!(!is_process_alive(0));
    }
}
