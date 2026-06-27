//! Hook 策略工厂 — 按格式创建对应安装策略

use super::plugin_models::hook_formats;
use super::strategies::{
    ClaudeMatcherStrategy, ClineHookStrategy, CodexHookStrategy, CopilotHookStrategy,
    FlatHookStrategy, HookInstallationStrategy, NestedHookStrategy,
};

/// 按格式创建 Hook 安装策略；不支持的格式返回 None
pub fn create(format: &str) -> Option<Box<dyn HookInstallationStrategy>> {
    if format.trim().is_empty() {
        return None;
    }
    match format.to_lowercase().as_str() {
        hook_formats::FLAT => Some(Box::new(FlatHookStrategy)),
        hook_formats::NESTED => Some(Box::new(NestedHookStrategy)),
        hook_formats::CODEX => Some(Box::new(CodexHookStrategy)),
        hook_formats::CLAUDE_MATCHER => Some(Box::new(ClaudeMatcherStrategy)),
        hook_formats::COPILOT => Some(Box::new(CopilotHookStrategy)),
        hook_formats::CLINE => Some(Box::new(ClineHookStrategy)),
        _ => None,
    }
}
