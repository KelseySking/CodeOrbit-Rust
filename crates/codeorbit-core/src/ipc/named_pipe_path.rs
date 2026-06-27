//! Named Pipe 路径生成

use std::env;

/// 环境变量覆盖管道名
pub const OVERRIDE_ENV: &str = "CodeOrbit_PIPE_NAME";

/// 默认管道名前缀
const DEFAULT_PREFIX: &str = "CodeOrbit";

/// 获取管道名称：优先读环境变量，否则 `CodeOrbit-{username}`
pub fn pipe_name() -> String {
    if let Ok(name) = env::var(OVERRIDE_ENV) {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let username = whoami::username().unwrap_or_else(|_| "user".to_string());
    format!("{DEFAULT_PREFIX}-{username}")
}

/// 获取完整管道路径
///
/// - Windows: `\\.\pipe\{name}`（与 C# 一致）
/// - Unix: 合法的 socket 绝对路径，见 [`unix_socket_path`]
pub fn full_path() -> String {
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\{}", pipe_name())
    }
    #[cfg(not(windows))]
    {
        unix_socket_path(&pipe_name())
    }
}

/// 为给定管道名计算 Unix domain socket 的绝对路径。
///
/// 规则（逻辑平台无关，便于在任意平台单测）：
/// - `name` 已是绝对路径 → 原样返回；
/// - `$XDG_RUNTIME_DIR` 存在 → `$XDG_RUNTIME_DIR/codeorbit/{name}.sock`；
/// - 否则 → `{temp_dir}/codeorbit-{name}.sock`。
#[cfg(any(unix, test))]
fn unix_socket_path(name: &str) -> String {
    if std::path::Path::new(name).is_absolute() {
        return name.to_string();
    }
    if let Ok(xdg) = env::var("XDG_RUNTIME_DIR")
        && !xdg.trim().is_empty()
    {
        return std::path::Path::new(xdg.trim())
            .join("codeorbit")
            .join(format!("{name}.sock"))
            .to_string_lossy()
            .into_owned();
    }
    env::temp_dir()
        .join(format!("codeorbit-{name}.sock"))
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_name_contains_prefix() {
        // ponytail: safe in test — single-threaded test runner, no other threads read env
        unsafe { env::remove_var(OVERRIDE_ENV) };
        let name = pipe_name();
        assert!(name.starts_with(DEFAULT_PREFIX));
    }

    #[test]
    fn override_env_takes_precedence() {
        unsafe {
            env::set_var(OVERRIDE_ENV, "  test-pipe  ");
        }
        assert_eq!(pipe_name(), "test-pipe");
        unsafe { env::remove_var(OVERRIDE_ENV) };
    }

    #[test]
    fn empty_override_falls_back_to_default() {
        unsafe {
            env::set_var(OVERRIDE_ENV, "   ");
        }
        let name = pipe_name();
        assert!(name.starts_with(DEFAULT_PREFIX));
        unsafe { env::remove_var(OVERRIDE_ENV) };
    }

    #[test]
    fn unix_socket_path_xdg_then_temp_fallback() {
        // 顺序测试两种情况，避免并行修改同一环境变量
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
        }
        let xdg = unix_socket_path("CodeOrbit-alice").replace('\\', "/");
        assert!(
            xdg.ends_with("codeorbit/CodeOrbit-alice.sock") && xdg.contains("/run/user/1000"),
            "应在 XDG/codeorbit 下: {xdg}"
        );

        unsafe { env::remove_var("XDG_RUNTIME_DIR") };
        let temp = unix_socket_path("CodeOrbit-bob").replace('\\', "/");
        assert!(
            temp.ends_with("codeorbit-CodeOrbit-bob.sock"),
            "temp 回退: {temp}"
        );
    }

    #[test]
    fn unix_socket_path_absolute_passes_through() {
        let abs = if cfg!(windows) {
            "C:/tmp/custom.sock"
        } else {
            "/tmp/custom.sock"
        };
        assert_eq!(unix_socket_path(abs), abs);
    }
}
