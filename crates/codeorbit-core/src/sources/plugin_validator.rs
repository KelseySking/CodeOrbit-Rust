//! 插件配置校验 — 安全性与正确性检查

use std::collections::HashMap;

use regex::Regex;

use super::hook_installation_utils;

// 限制
const MAX_PROCESS_NAMES: usize = 20;
const MAX_ENV_VAR_HINTS: usize = 10;
const MAX_PATH_PATTERNS: usize = 10;
const MAX_EVENTS: usize = 50;
const MIN_TIMEOUT_SECONDS: i32 = 1;
const MAX_TIMEOUT_SECONDS: i32 = 86400; // 24 小时

/// 标准事件名（大小写不敏感）
pub(crate) const STANDARD_EVENTS: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "UserPromptSubmit",
    "SessionStart",
    "SessionEnd",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "Notification",
    "PermissionRequest",
    "PostToolUseFailure",
    "PreCompact",
];

/// 校验检测规则
pub fn validate_detection(
    process_names: &[String],
    env_var_hints: &HashMap<String, String>,
    path_patterns: &[String],
    priority: i32,
) -> Result<(), String> {
    if process_names.len() > MAX_PROCESS_NAMES {
        return Err(format!("Too many process names (max {MAX_PROCESS_NAMES})"));
    }
    if env_var_hints.len() > MAX_ENV_VAR_HINTS {
        return Err(format!(
            "Too many environment variable hints (max {MAX_ENV_VAR_HINTS})"
        ));
    }
    if path_patterns.len() > MAX_PATH_PATTERNS {
        return Err(format!("Too many path patterns (max {MAX_PATH_PATTERNS})"));
    }

    let process_name_re = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();
    for name in process_names {
        if name.trim().is_empty() || name.len() > 64 {
            return Err(format!("Invalid process name: '{name}'"));
        }
        if !process_name_re.is_match(name) {
            return Err(format!(
                "Process name contains invalid characters: '{name}'"
            ));
        }
    }

    for (var_name, pattern) in env_var_hints {
        if var_name.trim().is_empty() {
            return Err("Environment variable name cannot be empty".to_string());
        }
        if !is_valid_env_var_name(var_name) {
            return Err(format!("Invalid environment variable name: '{var_name}'"));
        }
        validate_pattern(pattern)
            .map_err(|e| format!("Invalid env var pattern for '{var_name}': {e}"))?;
    }

    for pattern in path_patterns {
        validate_pattern(pattern).map_err(|e| format!("Invalid path pattern: {e}"))?;
    }

    if !(1..=1000).contains(&priority) {
        return Err(format!(
            "Priority must be between 1 and 1000 (got {priority})"
        ));
    }

    Ok(())
}

/// 校验 Hook 安装规格
pub fn validate_hook_installation(
    format: &str,
    config_path: &str,
    events: &[String],
    timeout_seconds: i32,
) -> Result<(), String> {
    // 允许任意非空格式；不支持的格式在安装时失败
    if format.trim().is_empty() {
        return Err("Hook format cannot be empty".to_string());
    }

    validate_config_path(config_path)?;

    if events.is_empty() {
        return Err("At least one event must be specified".to_string());
    }
    if events.len() > MAX_EVENTS {
        return Err(format!("Too many events (max {MAX_EVENTS})"));
    }
    for event_name in events {
        if !is_valid_standard_event(event_name) {
            return Err(format!("Invalid event name: '{event_name}'"));
        }
    }

    if !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
        return Err(format!(
            "Timeout must be between {MIN_TIMEOUT_SECONDS} and {MAX_TIMEOUT_SECONDS} seconds"
        ));
    }

    Ok(())
}

/// 校验配置文件路径的安全性
pub fn validate_config_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Config path cannot be empty".to_string());
    }

    // 必须以用户主目录标记开头
    if !path.starts_with("~/")
        && !path.starts_with("$HOME/")
        && !path.starts_with("%APPDATA%/")
        && !path.starts_with("%USERPROFILE%/")
    {
        return Err(
            "Config path must start with ~/,  $HOME/, %APPDATA%/, or %USERPROFILE%/".to_string(),
        );
    }

    // 禁止目录穿越
    if path.contains("..") {
        return Err("Config path cannot contain '..' (directory traversal)".to_string());
    }

    // 展开并检查实际路径（尽力而为）
    let expanded = hook_installation_utils::expand_path(path)
        .to_lowercase()
        .replace('\\', "/");

    const FORBIDDEN: &[&str] = &[
        "c:/windows/",
        "/windows/",
        "c:/system/",
        "/system/",
        "/etc/",
        "/usr/",
        "/var/",
        "c:/program files/",
        "c:/program files (x86)/",
    ];
    if FORBIDDEN.iter().any(|f| expanded.starts_with(f)) {
        return Err(format!("Config path cannot be in system directory: {path}"));
    }

    Ok(())
}

/// 校验模式（glob 或 regex）的安全性
fn validate_pattern(pattern: &str) -> Result<(), String> {
    if pattern.trim().is_empty() {
        return Err("Pattern cannot be empty".to_string());
    }

    // 危险的嵌套量词（ReDoS）
    if pattern.contains("(.*)*") || pattern.contains("(.+)+") || pattern.contains("(a|a)*") {
        return Err("Pattern contains dangerous nested quantifiers".to_string());
    }

    // 嵌套量词组过多
    if let Ok(re) = Regex::new(r"\(.*\)\*")
        && re.find_iter(pattern).count() > 2
    {
        return Err("Pattern has too many nested quantified groups".to_string());
    }

    // 若为 regex，尝试编译验证
    if (pattern.contains('^') || pattern.contains('$') || pattern.contains('\\'))
        && let Err(e) = Regex::new(pattern)
    {
        return Err(format!("Invalid regex pattern: {e}"));
    }

    Ok(())
}

/// 是否为合法的环境变量名
fn is_valid_env_var_name(name: &str) -> bool {
    if name.trim().is_empty() || name.len() > 255 {
        return false;
    }
    let re = Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").unwrap();
    re.is_match(name)
}

/// 是否为合法的标准事件名
fn is_valid_standard_event(event_name: &str) -> bool {
    STANDARD_EVENTS
        .iter()
        .any(|e| e.eq_ignore_ascii_case(event_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_must_start_with_home_marker() {
        assert!(validate_config_path("~/.codex/config.json").is_ok());
        assert!(validate_config_path("/etc/passwd").is_err());
        assert!(validate_config_path("~/../etc/x").is_err());
    }

    #[test]
    fn hook_installation_rejects_unknown_event() {
        let events = vec!["NotAnEvent".to_string()];
        assert!(validate_hook_installation("flat", "~/.x/y.json", &events, 10).is_err());
    }

    #[test]
    fn detection_rejects_bad_process_name() {
        let names = vec!["bad name!".to_string()];
        let res = validate_detection(&names, &HashMap::new(), &[], 100);
        assert!(res.is_err());
    }
}
