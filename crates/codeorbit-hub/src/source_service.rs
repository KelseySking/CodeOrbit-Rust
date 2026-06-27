//! SourceService — 数据源生命周期管理，映射到 API DTO（委托 ConfigInstaller）

use codeorbit_contracts::{
    RuntimeAssetsDto, SourceCapabilitiesDto, SourceDto, SourceOperationResultDto, SourceStatusDto,
};
use codeorbit_core::sources::SourcePluginLoader;
use codeorbit_core::sources::adapter_trait::SourceAdapter;

use crate::config_installer;

/// 默认能力（所有插件源均支持全部能力）
fn default_capabilities() -> SourceCapabilitiesDto {
    SourceCapabilitiesDto {
        hook_install: true,
        approval: true,
        question: true,
        transcript: true,
        always_allow: true,
    }
}

/// GET /api/sources 的数据
pub fn get_sources() -> Vec<SourceDto> {
    let loader = SourcePluginLoader::new();
    let plugins = loader.load_plugins();
    let bundled = loader.bundled_source_keys();

    let mut sources: Vec<SourceDto> = plugins
        .iter()
        .map(|adapter| {
            let key = adapter.source_key().to_string();
            let source_type = if bundled.contains(&key) {
                "bundled"
            } else {
                "user"
            };
            SourceDto {
                id: key.clone(),
                display_name: adapter.display_name().to_string(),
                icon_name: adapter.icon_name().to_string(),
                installed: config_installer::is_plugin_installed(&key),
                capabilities: default_capabilities(),
                source_type: source_type.to_string(),
            }
        })
        .collect();
    sources.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    sources
}

/// GET /api/sources/:source 的状态
pub fn get_source_status(source: &str) -> SourceStatusDto {
    let normalized = normalize_source(source);
    let loader = SourcePluginLoader::new();
    let plugins = loader.load_plugins();
    let plugin = plugins
        .iter()
        .find(|p| p.source_key().eq_ignore_ascii_case(&normalized));

    match plugin {
        None => SourceStatusDto {
            source: normalized.clone(),
            supported: false,
            installed: false,
            display_name: normalized,
        },
        Some(p) => SourceStatusDto {
            source: normalized.clone(),
            supported: true,
            installed: config_installer::is_plugin_installed(&normalized),
            display_name: p.display_name().to_string(),
        },
    }
}

pub fn install(source: &str) -> SourceOperationResultDto {
    run_source_operation(source, "installed", config_installer::install_plugin)
}

pub fn uninstall(source: &str) -> SourceOperationResultDto {
    run_source_operation(source, "uninstalled", config_installer::uninstall_plugin)
}

pub fn repair(source: &str) -> SourceOperationResultDto {
    run_source_operation(source, "repaired", config_installer::install_plugin)
}

/// 修复所有已安装的数据源
pub fn repair_all() -> bool {
    let loader = SourcePluginLoader::new();
    let mut all_ok = true;
    for plugin in loader.load_plugins() {
        let key = plugin.source_key().to_string();
        if config_installer::is_plugin_installed(&key) {
            all_ok &= config_installer::install_plugin(&key);
        }
    }
    all_ok
}

pub fn get_runtime_assets() -> RuntimeAssetsDto {
    RuntimeAssetsDto {
        runtime_directory: config_installer::runtime_directory()
            .to_string_lossy()
            .into_owned(),
        hook_script_path: config_installer::runtime_hook_script_path()
            .to_string_lossy()
            .into_owned(),
        bridge_exe_path: config_installer::runtime_bridge_exe_path()
            .to_string_lossy()
            .into_owned(),
        installed: config_installer::are_runtime_assets_installed(),
    }
}

pub fn repair_runtime_assets() -> bool {
    config_installer::repair_runtime_assets()
}

fn run_source_operation(
    source: &str,
    success_verb: &str,
    operation: impl Fn(&str) -> bool,
) -> SourceOperationResultDto {
    let normalized = normalize_source(source);
    let loader = SourcePluginLoader::new();
    let plugins = loader.load_plugins();
    let exists = plugins
        .iter()
        .any(|p| p.source_key().eq_ignore_ascii_case(&normalized));

    if !exists {
        return SourceOperationResultDto {
            source: normalized,
            success: false,
            installed: false,
            message: format!("Unsupported source: {source}"),
        };
    }

    let success = operation(&normalized);
    let message = if success {
        format!("{normalized} {success_verb}")
    } else {
        format!("{normalized} operation failed")
    };
    SourceOperationResultDto {
        source: normalized.clone(),
        success,
        installed: config_installer::is_plugin_installed(&normalized),
        message,
    }
}

fn normalize_source(source: &str) -> String {
    source.trim().to_lowercase()
}
