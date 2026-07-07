//! 被跟踪进程选择 — 为会话生命周期选择合适的进程（CLI > shell > 父进程）

use chrono::{DateTime, Utc};

use crate::process_ancestry::{process_stem, ProcessInfo};

/// 被跟踪进程
#[derive(Debug, Clone)]
pub struct TrackedProcess {
    pub pid: u32,
    pub kind: &'static str, // "cli" | "shell" | "host" | "parent"
    pub started_at_utc: Option<DateTime<Utc>>,
}

const SHELL_NAMES: &[&str] = &["pwsh", "powershell", "cmd", "bash", "zsh", "wsl", "nu"];

const TOOL_NAMES: &[&str] = &[
    "claude",
    "codex",
    "gemini",
    "cursor-agent",
    "qoder",
    "qoder-cli",
    "factory",
    "codebuddy",
    "opencode",
    "cline",
    "trae",
    "traecli",
    "copilot",
    "node",
];

/// 选择被跟踪进程
pub fn resolve(
    ancestry: &[ProcessInfo],
    parent_pid: u32,
    terminal_env: &[(String, String)],
) -> TrackedProcess {
    if let Some(cli) = find_tool_process(ancestry) {
        return TrackedProcess {
            pid: cli.pid,
            kind: "cli",
            started_at_utc: cli.started_at_utc,
        };
    }

    if is_vscode_terminal(terminal_env)
        && let Some(host) = find_vscode_host_process(ancestry)
    {
        return TrackedProcess {
            pid: host.pid,
            kind: "host",
            started_at_utc: host.started_at_utc,
        };
    }

    if let Some(shell) = find_shell_process(ancestry) {
        return TrackedProcess {
            pid: shell.pid,
            kind: "shell",
            started_at_utc: shell.started_at_utc,
        };
    }
    // 兜底：直接父进程
    let parent = ancestry.iter().find(|p| p.pid == parent_pid);
    TrackedProcess {
        pid: parent_pid,
        kind: "parent",
        started_at_utc: parent.and_then(|p| p.started_at_utc),
    }
}

fn find_shell_process(ancestry: &[ProcessInfo]) -> Option<&ProcessInfo> {
    ancestry.iter().find(|p| is_shell_process(p))
}

fn find_tool_process(ancestry: &[ProcessInfo]) -> Option<&ProcessInfo> {
    ancestry.iter().find(|p| is_tool_process(p))
}

fn find_vscode_host_process(ancestry: &[ProcessInfo]) -> Option<&ProcessInfo> {
    ancestry.iter().find(|p| is_vscode_host_process(p))
}

fn is_vscode_terminal(terminal_env: &[(String, String)]) -> bool {
    terminal_env.iter().any(|(key, value)| {
        (key == "TERM_PROGRAM" && value.eq_ignore_ascii_case("vscode"))
            || key == "VSCODE_INJECTION"
            || key == "VSCODE_GIT_IPC_HANDLE"
    })
}

fn is_shell_process(process: &ProcessInfo) -> bool {
    let name = process_stem(&process.name).to_lowercase();
    SHELL_NAMES.contains(&name.as_str())
}

fn is_vscode_host_process(process: &ProcessInfo) -> bool {
    let name = process_stem(&process.name).to_lowercase();
    matches!(name.as_str(), "code" | "code - insiders" | "vscodium")
}

fn is_tool_process(process: &ProcessInfo) -> bool {
    let name = process_stem(&process.name).to_lowercase();
    if TOOL_NAMES.contains(&name.as_str()) {
        return true;
    }
    let path = process.executable_path.to_lowercase();
    [
        "claude",
        "codex",
        "gemini",
        "opencode",
        "cline",
        "cursor-agent",
    ]
    .iter()
    .any(|needle| path.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, name: &str, exe: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid: 0,
            name: name.to_string(),
            executable_path: exe.to_string(),
            started_at_utc: None,
        }
    }

    #[test]
    fn shell_used_when_no_tool() {
        let ancestry = vec![proc(10, "git.exe", ""), proc(20, "bash", "/bin/bash")];
        let tracked = resolve(&ancestry, 99, &[]);
        assert_eq!(tracked.pid, 20);
        assert_eq!(tracked.kind, "shell");
    }

    #[test]
    fn tool_chosen_when_no_shell() {
        let ancestry = vec![proc(10, "claude", "/usr/bin/claude")];
        let tracked = resolve(&ancestry, 99, &[]);
        assert_eq!(tracked.pid, 10);
        assert_eq!(tracked.kind, "cli");
    }

    #[test]
    fn parent_fallback() {
        let ancestry = vec![proc(10, "git.exe", "")];
        let tracked = resolve(&ancestry, 10, &[]);
        assert_eq!(tracked.pid, 10);
        assert_eq!(tracked.kind, "parent");
    }

    #[test]
    fn cli_preferred_over_shell() {
        let ancestry = vec![
            proc(10, "claude", "/usr/bin/claude"),
            proc(20, "bash", "/bin/bash"),
        ];
        let tracked = resolve(&ancestry, 99, &[]);
        assert_eq!(tracked.kind, "cli");
        assert_eq!(tracked.pid, 10);
    }

    #[test]
    fn cli_preferred_over_hook_shell() {
        let ancestry = vec![
            proc(10, "cmd.exe", "C:/Windows/System32/cmd.exe"),
            proc(
                20,
                "codex.exe",
                "C:/Users/test/AppData/Roaming/npm/codex.exe",
            ),
            proc(30, "pwsh.exe", "C:/Program Files/PowerShell/7/pwsh.exe"),
        ];
        let tracked = resolve(&ancestry, 10, &[]);
        assert_eq!(tracked.kind, "cli");
        assert_eq!(tracked.pid, 20);
    }

    #[test]
    fn vscode_terminal_prefers_cli() {
        let ancestry = vec![
            proc(10, "claude", "/usr/bin/claude"),
            proc(20, "pwsh.exe", "C:/Program Files/PowerShell/7/pwsh.exe"),
            proc(
                30,
                "Code.exe",
                "C:/Users/test/AppData/Local/Programs/Microsoft VS Code/Code.exe",
            ),
        ];
        let env = vec![("TERM_PROGRAM".to_string(), "vscode".to_string())];
        let tracked = resolve(&ancestry, 10, &env);
        assert_eq!(tracked.kind, "cli");
        assert_eq!(tracked.pid, 10);
    }

    #[test]
    fn vscode_terminal_uses_host_when_tool_missing() {
        let ancestry = vec![proc(
            30,
            "Code.exe",
            "C:/Users/test/AppData/Local/Programs/Microsoft VS Code/Code.exe",
        )];
        let env = vec![("VSCODE_INJECTION".to_string(), "1".to_string())];
        let tracked = resolve(&ancestry, 10, &env);
        assert_eq!(tracked.kind, "host");
        assert_eq!(tracked.pid, 30);
    }
}
