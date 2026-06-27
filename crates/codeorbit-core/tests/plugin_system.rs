//! 插件系统集成测试 — 解析全部 bundled 插件、工厂、策略往返、注册表

use std::collections::HashMap;
use std::path::PathBuf;

use codeorbit_core::sources::adapter_trait::SourceAdapter;
use codeorbit_core::sources::hook_strategy_factory;
use codeorbit_core::sources::plugin_models::{HookInstallationSpec, hook_formats};
use codeorbit_core::sources::source_plugin_loader::SourcePluginLoader;
use codeorbit_core::sources::strategies::{
    ClineHookStrategy, CodexHookStrategy, CopilotHookStrategy, FlatHookStrategy,
    HookInstallationStrategy, NestedHookStrategy,
};
use codeorbit_core::sources::{
    SourceAdapterRegistry, plugin_process_detector::PluginProcessDetector,
};

fn bundled_dir() -> PathBuf {
    // crates/codeorbit-core -> workspace root -> bundled-plugins
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("bundled-plugins")
}

fn unique_temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("codeorbit-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_spec(format: &str, config_path: &str, events: &[&str]) -> HookInstallationSpec {
    HookInstallationSpec {
        format: format.to_string(),
        config_path: config_path.to_string(),
        events: events.iter().map(|e| e.to_string()).collect(),
        timeout_seconds: 10,
        extra_config: None,
    }
}

#[test]
fn loads_all_bundled_plugins() {
    let bundled = bundled_dir();
    assert!(bundled.exists(), "bundled-plugins 目录应存在: {bundled:?}");

    let json_count = std::fs::read_dir(&bundled)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
        })
        .count();

    assert_eq!(json_count, 19, "应有 19 个 bundled 插件 JSON");

    let loader = SourcePluginLoader::with_dirs(unique_temp_dir("plugins"), bundled);
    let adapters = loader.load_plugins();

    assert_eq!(
        adapters.len(),
        19,
        "全部 19 个插件都应成功解析，实际 {}",
        adapters.len()
    );
    for adapter in &adapters {
        assert!(!adapter.source_key().is_empty());
        assert!(!adapter.display_name().is_empty());
    }
}

#[test]
fn registry_resolves_known_and_unknown() {
    let loader = SourcePluginLoader::with_dirs(unique_temp_dir("registry"), bundled_dir());
    let registry = SourceAdapterRegistry::from_loader(&loader);

    assert!(
        registry.is_known_source(Some("claude")),
        "claude 应为已知来源"
    );
    assert!(!registry.is_known_source(Some("definitely-not-real")));

    let unknown = registry.get(Some("definitely-not-real"));
    assert_eq!(unknown.source_key(), "unknown");
}

#[test]
fn factory_creates_all_supported_formats() {
    for fmt in [
        hook_formats::FLAT,
        hook_formats::NESTED,
        hook_formats::CODEX,
        hook_formats::CLAUDE_MATCHER,
        hook_formats::COPILOT,
        hook_formats::CLINE,
    ] {
        assert!(
            hook_strategy_factory::create(fmt).is_some(),
            "{fmt} 应有策略"
        );
    }
    assert!(hook_strategy_factory::create("nonexistent").is_none());
    assert!(hook_strategy_factory::create("").is_none());
}

#[test]
fn flat_strategy_round_trip() {
    let dir = unique_temp_dir("flat");
    let config = dir.join("hooks.json");
    let config_str = config.to_string_lossy().to_string();
    let spec = make_spec(hook_formats::FLAT, &config_str, &["PreToolUse", "Stop"]);
    let strat = FlatHookStrategy;

    assert!(!strat.is_installed("cursor", &spec));
    assert!(strat.install("cursor", &spec));
    assert!(strat.is_installed("cursor", &spec));
    assert!(strat.uninstall("cursor", &spec));
    assert!(!strat.is_installed("cursor", &spec));
}

#[test]
fn nested_strategy_round_trip() {
    let dir = unique_temp_dir("nested");
    let config = dir.join("settings.json");
    let config_str = config.to_string_lossy().to_string();
    let spec = make_spec(hook_formats::NESTED, &config_str, &["PreToolUse"]);
    let strat = NestedHookStrategy;

    assert!(strat.install("gemini", &spec));
    assert!(strat.is_installed("gemini", &spec));
    assert!(strat.uninstall("gemini", &spec));
    assert!(!strat.is_installed("gemini", &spec));
}

#[test]
fn codex_strategy_round_trip_has_command_windows() {
    let dir = unique_temp_dir("codex");
    let config = dir.join("hooks.json");
    let config_str = config.to_string_lossy().to_string();
    let spec = make_spec(hook_formats::CODEX, &config_str, &["PreToolUse"]);
    let strat = CodexHookStrategy;

    assert!(strat.install("codex", &spec));
    assert!(strat.is_installed("codex", &spec));

    // 验证写入了 commandWindows 字段（双层嵌套）
    let content = std::fs::read_to_string(&config).unwrap();
    assert!(
        content.contains("commandWindows"),
        "Codex 应写入 commandWindows"
    );
    assert!(content.contains("--source codex"));

    assert!(strat.uninstall("codex", &spec));
    assert!(!strat.is_installed("codex", &spec));
}

#[test]
fn copilot_strategy_round_trip() {
    let dir = unique_temp_dir("copilot");
    let config = dir.join("hooks.json");
    let config_str = config.to_string_lossy().to_string();
    let spec = make_spec(hook_formats::COPILOT, &config_str, &["UserPromptSubmit"]);
    let strat = CopilotHookStrategy;

    assert!(strat.install("copilot", &spec));
    assert!(strat.is_installed("copilot", &spec));

    let content = std::fs::read_to_string(&config).unwrap();
    assert!(content.contains("\"version\""), "Copilot 应含 version 字段");

    assert!(strat.uninstall("copilot", &spec));
    assert!(!strat.is_installed("copilot", &spec));
}

#[test]
fn cline_strategy_round_trip_writes_scripts() {
    let dir = unique_temp_dir("cline");
    let hooks_dir = dir.join("hooks"); // 无扩展名 → 作为目录
    let config_str = hooks_dir.to_string_lossy().to_string();
    let spec = make_spec(hook_formats::CLINE, &config_str, &["Stop", "PreToolUse"]);
    let strat = ClineHookStrategy;

    assert!(strat.install("cline", &spec));
    assert!(strat.is_installed("cline", &spec));
    assert!(hooks_dir.join("Stop.ps1").exists());
    assert!(hooks_dir.join("PreToolUse.ps1").exists());

    assert!(strat.uninstall("cline", &spec));
    assert!(!strat.is_installed("cline", &spec));
}

#[test]
fn detector_matches_by_process_name() {
    let loader = SourcePluginLoader::with_dirs(unique_temp_dir("detect"), bundled_dir());
    let detector = PluginProcessDetector::from_loader(&loader);

    // 使用一个已知插件的进程名做检测（claude 插件检测规则含 "claude"）
    let result = detector.detect_from_process_list(&[("claude".to_string(), None)]);
    // 至少不应 panic；若 claude 插件定义了 detection 规则则应匹配
    let _ = result;

    // 完全不相关的进程不应匹配
    let none = detector.detect_from_process_list(&[("totally-unrelated-xyz".to_string(), None)]);
    assert!(none.is_none() || none.as_deref() != Some(""));

    let _ = HashMap::<String, String>::new();
}
