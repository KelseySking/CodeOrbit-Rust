//! 插件配置校验集成测试 — ID 唯一性、必需字段、数量

use std::collections::HashSet;
use std::path::PathBuf;

use codeorbit_core::sources::SourcePluginLoader;
use codeorbit_core::sources::adapter_trait::SourceAdapter;

fn bundled_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("bundled-plugins")
}

fn loader() -> SourcePluginLoader {
    let temp = std::env::temp_dir().join(format!("codeorbit-plugval-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let _ = std::fs::create_dir_all(&temp);
    SourcePluginLoader::with_dirs(temp, bundled_dir())
}

#[test]
fn all_plugin_ids_are_unique_and_count_19() {
    let adapters = loader().load_plugins();
    let keys: HashSet<String> = adapters
        .iter()
        .map(|a| a.source_key().to_string())
        .collect();

    assert_eq!(adapters.len(), 19, "应加载 19 个插件");
    assert_eq!(keys.len(), adapters.len(), "插件 ID 必须唯一");
}

#[test]
fn all_plugins_have_required_fields() {
    for adapter in loader().load_plugins() {
        assert!(!adapter.source_key().is_empty(), "source_key 不能为空");
        assert!(
            !adapter.display_name().is_empty(),
            "display_name 不能为空: {}",
            adapter.source_key()
        );
        assert!(
            !adapter.icon_name().is_empty(),
            "icon_name 不能为空: {}",
            adapter.source_key()
        );
    }
}

#[test]
fn bundled_source_keys_match_count() {
    let keys = loader().bundled_source_keys();
    assert_eq!(keys.len(), 19, "bundled source keys 应为 19");
}
