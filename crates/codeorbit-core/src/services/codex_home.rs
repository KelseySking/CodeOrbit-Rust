//! Codex home 目录解析（对齐 C# ConfigInstaller.ResolveCodexHome）

use std::path::PathBuf;

/// 解析 Codex home 目录：`$CODEX_HOME`（支持 `~` 展开），默认 `~/.codex`
pub fn resolve_codex_home() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let raw = std::env::var("CODEX_HOME").unwrap_or_default();
    let raw = raw.trim();

    if raw.is_empty() {
        return home.join(".codex");
    }
    if raw == "~" {
        return home;
    }
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        return home.join(rest);
    }
    PathBuf::from(raw)
}
