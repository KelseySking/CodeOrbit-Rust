//! 插件发现与加载 — 从 JSON 文件加载 CLI 源插件

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::adapter_trait::PluginSourceAdapter;
use super::source_plugin_json_parser::{self, ParseError};

/// 发现并加载 CLI 源插件
pub struct SourcePluginLoader {
    plugin_directory: PathBuf,
    bundled_directory: PathBuf,
}

impl SourcePluginLoader {
    /// 使用默认目录创建（用户目录 + exe 同级 bundled-plugins）
    pub fn new() -> Self {
        Self {
            plugin_directory: default_plugin_directory(),
            bundled_directory: bundled_plugin_directory(),
        }
    }

    /// 显式指定用户目录与 bundled 目录（便于测试）
    pub fn with_dirs(plugin_directory: PathBuf, bundled_directory: PathBuf) -> Self {
        Self {
            plugin_directory,
            bundled_directory,
        }
    }

    /// 加载所有有效插件；无效插件跳过并记录日志。bundled 优先且不可被覆盖。
    pub fn load_plugins(&self) -> Vec<PluginSourceAdapter> {
        let mut adapters: Vec<PluginSourceAdapter> = Vec::new();
        let mut loaded_keys: HashSet<String> = HashSet::new();

        self.load_bundled_plugins(&mut adapters, &mut loaded_keys);
        self.load_user_plugins(&mut adapters, &mut loaded_keys);

        adapters
    }

    /// 返回来自 bundled 插件的 source key 集合
    pub fn bundled_source_keys(&self) -> HashSet<String> {
        let mut keys = HashSet::new();
        if !self.bundled_directory.exists() {
            return keys;
        }
        for file in json_files(&self.bundled_directory) {
            if let Ok(adapter) = self.try_load_plugin_from_file(&file, &[]) {
                use super::adapter_trait::SourceAdapter;
                keys.insert(adapter.source_key().to_string());
            }
        }
        keys
    }

    fn load_bundled_plugins(
        &self,
        adapters: &mut Vec<PluginSourceAdapter>,
        loaded_keys: &mut HashSet<String>,
    ) {
        if !self.bundled_directory.exists() {
            return;
        }
        let existing: Vec<String> = loaded_keys.iter().cloned().collect();
        for file in json_files(&self.bundled_directory) {
            match self.try_load_plugin_from_file(&file, &existing) {
                Ok(adapter) => {
                    use super::adapter_trait::SourceAdapter;
                    loaded_keys.insert(adapter.source_key().to_string());
                    adapters.push(adapter);
                }
                Err(err) => {
                    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    tracing::error!("Bundled plugin '{name}': {}", err.message);
                }
            }
        }
    }

    fn load_user_plugins(
        &self,
        adapters: &mut Vec<PluginSourceAdapter>,
        loaded_keys: &mut HashSet<String>,
    ) {
        if !self.plugin_directory.exists() {
            // 创建空目录后返回（无用户插件）
            let _ = std::fs::create_dir_all(&self.plugin_directory);
            return;
        }

        for file in json_files(&self.plugin_directory) {
            let existing: Vec<String> = loaded_keys.iter().cloned().collect();
            match self.try_load_plugin_from_file(&file, &existing) {
                Ok(adapter) => {
                    use super::adapter_trait::SourceAdapter;
                    loaded_keys.insert(adapter.source_key().to_string());
                    adapters.push(adapter);
                }
                Err(err) => {
                    use super::plugin_models::PluginValidationError;
                    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    if err.kind == PluginValidationError::DuplicateSourceKey {
                        tracing::warn!("Plugin '{name}': {} (skipped)", err.message);
                    } else {
                        tracing::error!("Plugin '{name}': {} (skipped)", err.message);
                    }
                }
            }
        }
    }

    /// 尝试从单个文件加载插件
    pub fn try_load_plugin_from_file(
        &self,
        file_path: &Path,
        existing_keys: &[String],
    ) -> Result<PluginSourceAdapter, ParseError> {
        use super::plugin_models::PluginValidationError;

        let content = std::fs::read_to_string(file_path).map_err(|e| ParseError {
            message: format!("Failed to read file: {e}"),
            kind: PluginValidationError::InvalidJson,
        })?;

        let metadata = source_plugin_json_parser::parse(&content, existing_keys)?;

        PluginSourceAdapter::new(
            metadata.source_key,
            metadata.display_name,
            metadata.icon_name,
            metadata.permission_response_style,
            metadata.event_mappings,
            metadata.detection,
            metadata.hook_installation,
            Some(file_path.to_string_lossy().into_owned()),
        )
        .map_err(|e| ParseError {
            message: e,
            kind: PluginValidationError::InvalidJson,
        })
    }
}

impl Default for SourcePluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// 用户插件目录：<config>/CodeOrbit/sources
fn default_plugin_directory() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CodeOrbit")
        .join("sources")
}

/// bundled 插件目录：优先环境变量 `CodeOrbit_BUNDLED_PLUGINS_DIR`，否则 exe 同级 bundled-plugins
fn bundled_plugin_directory() -> PathBuf {
    if let Ok(dir) = std::env::var("CodeOrbit_BUNDLED_PLUGINS_DIR")
        && !dir.is_empty()
    {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bundled-plugins")
}

/// 列出目录下所有 *.json 文件（仅顶层），按名称排序
fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("json"))
                        .unwrap_or(false)
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    files
}
