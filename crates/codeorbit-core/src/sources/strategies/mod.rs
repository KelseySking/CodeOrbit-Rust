//! Hook 安装策略 — 6 种格式的 install/uninstall/is_installed 实现

pub mod claude_matcher;
pub mod cline;
pub mod codex;
pub mod copilot;
pub mod flat;
pub mod nested;

use std::fs;

use super::hook_installation_utils;
use super::plugin_models::{ExtraConfigSpec, HookInstallationSpec};

pub use claude_matcher::ClaudeMatcherStrategy;
pub use cline::ClineHookStrategy;
pub use codex::CodexHookStrategy;
pub use copilot::CopilotHookStrategy;
pub use flat::FlatHookStrategy;
pub use nested::NestedHookStrategy;

/// 特定格式的 Hook 安装策略
pub trait HookInstallationStrategy {
    /// 为给定来源安装 hook
    fn install(&self, source_key: &str, spec: &HookInstallationSpec) -> bool;
    /// 卸载给定来源的 hook
    fn uninstall(&self, source_key: &str, spec: &HookInstallationSpec) -> bool;
    /// 检查 hook 是否已安装
    fn is_installed(&self, source_key: &str, spec: &HookInstallationSpec) -> bool;
}

/// 判断某命令字符串是否为 CodeOrbit hook 命令（大小写不敏感，对齐 C# OrdinalIgnoreCase）
pub(crate) fn is_codeorbit_hook_command(command: &str, source_key: &str) -> bool {
    let lc = command.to_lowercase();
    lc.contains("codeorbit.bridge")
        || lc.contains("codeorbit-bridge")
        || lc.contains(&format!("--source {source_key}").to_lowercase())
}

/// flat / nested 共享的 TOML extra_config 安装：在指定 section 下追加 key = value
pub(crate) fn install_extra_config_toml(extra: &ExtraConfigSpec) {
    let file_path = hook_installation_utils::expand_path(&extra.file);
    hook_installation_utils::ensure_directory_exists(&file_path);

    if !file_path.to_lowercase().ends_with(".toml") {
        return;
    }

    let mut lines: Vec<String> = if std::path::Path::new(&file_path).exists() {
        fs::read_to_string(&file_path)
            .map(|c| c.lines().map(str::to_string).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let section_line = extra.section.clone().unwrap_or_default();
    let mut section_index = lines.iter().position(|l| l.trim() == section_line);

    if section_index.is_none() && !section_line.is_empty() {
        lines.push(String::new());
        lines.push(section_line.clone());
        section_index = Some(lines.len() - 1);
    }

    let key_line = format!("{} = {}", extra.key, extra.value);
    let key_prefix = format!("{} =", extra.key);
    let key_exists = lines.iter().any(|l| l.trim().starts_with(&key_prefix));

    if !key_exists {
        match section_index {
            Some(idx) => lines.insert(idx + 1, key_line),
            None => lines.push(key_line),
        }
    }

    let _ = fs::write(&file_path, lines.join("\n"));
}
