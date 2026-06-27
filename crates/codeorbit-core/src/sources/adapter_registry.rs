//! 源适配器注册表 — 注册、查找，未知来源回退

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use super::adapter_trait::{BuiltInSourceAdapter, PluginSourceAdapter, SourceAdapter};
use super::plugin_models::PermissionResponseStyle;
use super::source_plugin_loader::SourcePluginLoader;

/// 全局惰性注册表（对齐 C# `CodeOrbitSourceAdapterRegistry` 静态语义）。
/// 首次访问时从默认目录加载全部插件。
pub fn global() -> &'static SourceAdapterRegistry {
    static INSTANCE: OnceLock<SourceAdapterRegistry> = OnceLock::new();
    INSTANCE.get_or_init(|| SourceAdapterRegistry::from_loader(&SourcePluginLoader::new()))
}

/// 源适配器注册表
pub struct SourceAdapterRegistry {
    adapters: HashMap<String, Arc<dyn SourceAdapter>>,
    unknown: Arc<dyn SourceAdapter>,
}

impl SourceAdapterRegistry {
    /// 从已加载的插件适配器构造
    pub fn from_adapters(plugins: Vec<PluginSourceAdapter>) -> Self {
        let mut adapters: HashMap<String, Arc<dyn SourceAdapter>> = HashMap::new();
        for plugin in plugins {
            let key = plugin.source_key().to_lowercase();
            if adapters.contains_key(&key) {
                tracing::warn!(
                    "Plugin '{}' conflicts with existing source (skipped)",
                    plugin.source_key()
                );
                continue;
            }
            adapters.insert(key, Arc::new(plugin));
        }

        Self {
            adapters,
            unknown: Arc::new(BuiltInSourceAdapter::new(
                "unknown",
                "未知工具",
                "unknown",
                PermissionResponseStyle::ClaudeStyle,
            )),
        }
    }

    /// 从加载器构造（加载全部插件）
    pub fn from_loader(loader: &SourcePluginLoader) -> Self {
        Self::from_adapters(loader.load_plugins())
    }

    /// 已知来源 key 列表
    pub fn known_sources(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }

    /// 是否为已知来源
    pub fn is_known_source(&self, source: Option<&str>) -> bool {
        self.try_get(source).is_some()
    }

    /// 查找适配器；未找到返回 None
    pub fn try_get(&self, source: Option<&str>) -> Option<&Arc<dyn SourceAdapter>> {
        let key = source?.trim().to_lowercase();
        if key.is_empty() {
            return None;
        }
        self.adapters.get(&key)
    }

    /// 获取适配器；未找到返回 unknown 回退
    pub fn get(&self, source: Option<&str>) -> Arc<dyn SourceAdapter> {
        self.try_get(source)
            .cloned()
            .unwrap_or_else(|| Arc::clone(&self.unknown))
    }
}
