//! 跨平台进程祖先链遍历（基于 sysinfo，内部在 Linux 走 /proc、Windows/macOS 走系统 API）

use chrono::{DateTime, Utc};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// 进程信息
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    /// 父进程 PID（spec 数据字段，遍历用局部变量；保留以符合 ProcessInfo 契约）
    #[allow(dead_code)]
    pub parent_pid: u32,
    pub name: String,
    pub executable_path: String,
    pub started_at_utc: Option<DateTime<Utc>>,
}

/// 进程名去扩展名 + 去空白（大小写保持）
pub fn process_stem(name: &str) -> String {
    let base = name
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim();
    match base.rfind('.') {
        Some(idx) if idx > 0 => base[..idx].to_string(),
        _ => base.to_string(),
    }
}

/// 获取当前进程的父进程 PID
pub fn get_parent_pid() -> u32 {
    let current = std::process::id();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(current)]),
        true,
        ProcessRefreshKind::everything(),
    );
    sys.process(Pid::from_u32(current))
        .and_then(|p| p.parent())
        .map(|p| p.as_u32())
        .unwrap_or(0)
}

/// 向上遍历进程祖先链：从 `starting_pid` 到最远祖先（最多 `max_depth` 层）
pub fn build_ancestry(starting_pid: u32, max_depth: usize) -> Vec<ProcessInfo> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    let mut ancestry = Vec::new();
    let mut pid = starting_pid;
    for _ in 0..max_depth {
        if pid == 0 {
            break;
        }
        let Some(proc) = sys.process(Pid::from_u32(pid)) else {
            break;
        };
        let parent_pid = proc.parent().map(|p| p.as_u32()).unwrap_or(0);
        ancestry.push(ProcessInfo {
            pid,
            parent_pid,
            name: proc.name().to_string_lossy().into_owned(),
            executable_path: proc
                .exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            started_at_utc: DateTime::from_timestamp(proc.start_time() as i64, 0),
        });
        pid = parent_pid;
    }
    ancestry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_strips_extension_and_path() {
        assert_eq!(process_stem("node.exe"), "node");
        assert_eq!(process_stem("/usr/bin/bash"), "bash");
        assert_eq!(process_stem("C:\\tools\\claude.exe"), "claude");
        assert_eq!(process_stem(".hidden"), ".hidden");
    }

    #[test]
    fn ancestry_includes_current_process() {
        let pid = std::process::id();
        let ancestry = build_ancestry(pid, 12);
        assert!(!ancestry.is_empty());
        assert_eq!(ancestry[0].pid, pid);
        assert!(ancestry.len() <= 12);
    }

    #[test]
    fn parent_pid_is_nonzero_for_test_process() {
        // 测试进程总有父进程（cargo/test runner）
        assert_ne!(get_parent_pid(), 0);
    }
}
