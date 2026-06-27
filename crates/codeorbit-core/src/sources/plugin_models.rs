//! 插件系统核心数据模型 — 元数据、检测规则、Hook 安装规格

use std::collections::HashMap;

use regex::RegexBuilder;

/// 插件 permission 响应风格
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionResponseStyle {
    ClaudeStyle,
    Codex,
}

/// 加载插件时可能出现的具体校验错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginValidationError {
    None,
    InvalidJson,
    MissingRequiredField,
    InvalidSchemaVersion,
    InvalidSourceKey,
    DuplicateSourceKey,
    ConflictWithBuiltInSource,
    InvalidPermissionStyle,
    InvalidEventMapping,
}

/// 用于检测匹配的进程信息
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: i32,
    pub parent_pid: i32,
    pub name: String,
    pub executable_path: Option<String>,
}

impl ProcessInfo {
    pub fn new(name: impl Into<String>, executable_path: Option<String>) -> Self {
        Self {
            pid: 0,
            parent_pid: 0,
            name: name.into(),
            executable_path,
        }
    }
}

/// 插件定义的 CLI 源检测规则
#[derive(Debug, Clone)]
pub struct DetectionRule {
    pub source_key: String,
    pub process_names: Vec<String>,
    pub env_var_hints: HashMap<String, String>,
    pub path_patterns: Vec<String>,
    pub priority: i32,
}

impl DetectionRule {
    /// 测试此检测规则是否匹配给定的进程祖先链
    pub fn matches(&self, ancestry: &[ProcessInfo]) -> bool {
        for process in ancestry {
            // 进程名匹配（去掉扩展名，大小写不敏感）
            let process_name = file_name_without_extension(&process.name);
            if self
                .process_names
                .iter()
                .any(|pn| pn.eq_ignore_ascii_case(&process_name))
            {
                return true;
            }

            // 路径模式匹配
            if let Some(path) = &process.executable_path
                && self
                    .path_patterns
                    .iter()
                    .any(|pattern| matches_glob_pattern(path, pattern))
            {
                return true;
            }
        }

        // 环境变量匹配
        if !self.env_var_hints.is_empty() && self.matches_env_vars() {
            return true;
        }

        false
    }

    fn matches_env_vars(&self) -> bool {
        for (var_name, pattern) in &self.env_var_hints {
            if let Ok(value) = std::env::var(var_name)
                && matches_value_pattern(&value, pattern)
            {
                return true;
            }
        }
        false
    }
}

/// 去掉文件扩展名（等价于 C# Path.GetFileNameWithoutExtension 对纯文件名的处理）
fn file_name_without_extension(name: &str) -> String {
    // 仅取最后一段（兼容传入完整路径的情况）
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match base.rfind('.') {
        Some(idx) if idx > 0 => base[..idx].to_string(),
        _ => base.to_string(),
    }
}

/// 值模式匹配（glob 或 regex），用于环境变量值
fn matches_value_pattern(value: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return !value.is_empty();
    }

    // 含 regex 特殊字符时优先按 regex 处理
    if pattern.contains('^') || pattern.contains('$') || pattern.contains('\\') {
        return match RegexBuilder::new(pattern).build() {
            Ok(re) => re.is_match(value),
            Err(_) => false,
        };
    }

    // 回退为简单 glob（大小写不敏感）
    let regex_pattern = format!(
        "^{}$",
        regex::escape(pattern)
            .replace("\\*", ".*")
            .replace("\\?", ".")
    );
    match RegexBuilder::new(&regex_pattern)
        .case_insensitive(true)
        .build()
    {
        Ok(re) => re.is_match(value),
        Err(_) => false,
    }
}

/// 路径 glob 匹配（`**` 任意目录深度，`*` 单层，`?` 单字符）
fn matches_glob_pattern(path: &str, pattern: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    let path = path.replace('\\', "/").to_lowercase();
    let pattern = pattern.replace('\\', "/").to_lowercase();

    let regex_pattern = format!(
        "^{}$",
        regex::escape(&pattern)
            .replace("\\*\\*/", "(.*/)?") // ** 匹配任意目录深度
            .replace("\\*", "[^/]*") // * 在目录内匹配
            .replace("\\?", ".") // ? 匹配单字符
    );

    match RegexBuilder::new(&regex_pattern).build() {
        Ok(re) => re.is_match(&path),
        Err(_) => false,
    }
}

/// Hook 安装规格
#[derive(Debug, Clone)]
pub struct HookInstallationSpec {
    pub format: String,
    pub config_path: String,
    pub events: Vec<String>,
    pub timeout_seconds: i32,
    pub extra_config: Option<ExtraConfigSpec>,
}

/// 额外配置文件规格（如 Codex 的 config.toml）
#[derive(Debug, Clone)]
pub struct ExtraConfigSpec {
    pub file: String,
    pub section: Option<String>,
    pub key: String,
    pub value: String,
}

/// 支持的 Hook 安装格式
pub mod hook_formats {
    /// 扁平数组格式: [{event, command, timeout}] — Cursor, Trae
    pub const FLAT: &str = "flat";
    /// 嵌套格式: {hooks: {EventName: [{command, timeout}]}} — Gemini
    pub const NESTED: &str = "nested";
    /// Codex 格式: {hooks: {EventName: [{hooks: [{type, command, ...}]}]}}
    pub const CODEX: &str = "codex";
    /// Claude matcher 格式: {hooks: {EventName: [{matcher, hooks: [...]}]}}
    pub const CLAUDE_MATCHER: &str = "claude-matcher";
    /// Copilot 格式: {version, hooks: [{event, command, timeout}]}
    pub const COPILOT: &str = "copilot";
    /// Cline 格式: 每事件一个 PowerShell 脚本
    pub const CLINE: &str = "cline";

    /// 检查格式是否受支持
    pub fn is_supported(format: &str) -> bool {
        matches!(
            format.to_lowercase().as_str(),
            FLAT | NESTED | CODEX | CLAUDE_MATCHER | COPILOT | CLINE
        )
    }
}

/// 从插件 JSON 中提取的元数据
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub source_key: String,
    pub display_name: String,
    pub icon_name: String,
    pub permission_response_style: PermissionResponseStyle,
    pub event_mappings: HashMap<String, String>,
    pub detection: Option<DetectionRule>,
    pub hook_installation: Option<HookInstallationSpec>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_without_extension_strips_ext() {
        assert_eq!(file_name_without_extension("claude.exe"), "claude");
        assert_eq!(file_name_without_extension("/usr/bin/node"), "node");
        assert_eq!(file_name_without_extension("C:\\tools\\codex.cmd"), "codex");
        assert_eq!(file_name_without_extension(".hidden"), ".hidden");
    }

    #[test]
    fn process_name_match_is_case_insensitive() {
        let rule = DetectionRule {
            source_key: "claude".into(),
            process_names: vec!["claude".into()],
            env_var_hints: HashMap::new(),
            path_patterns: vec![],
            priority: 100,
        };
        let ancestry = vec![ProcessInfo::new("Claude.exe", None)];
        assert!(rule.matches(&ancestry));
    }

    #[test]
    fn path_glob_matches_double_star() {
        assert!(matches_glob_pattern(
            "/home/user/.local/bin/codex",
            "**/bin/codex"
        ));
        assert!(matches_glob_pattern(
            "C:\\Program Files\\app\\cli.exe",
            "**/cli.exe"
        ));
        assert!(!matches_glob_pattern("/home/user/other", "**/bin/codex"));
    }

    #[test]
    fn value_pattern_star_matches_nonempty() {
        assert!(matches_value_pattern("anything", "*"));
        assert!(!matches_value_pattern("", "*"));
    }
}
