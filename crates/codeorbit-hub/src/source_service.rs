//! SourceService — 数据源生命周期管理，映射到 API DTO（委托 ConfigInstaller）

use codeorbit_contracts::{
    RuntimeAssetsDto, SourceCapabilitiesDto, SourceDto, SourceOperationResultDto, SourceStatusDto,
    WslDistrosDto,
};
use codeorbit_core::sources::SourcePluginLoader;
use codeorbit_core::sources::adapter_trait::SourceAdapter;

use crate::config_installer;
use crate::wsl_installer;

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
            distro: None,
            probe_ok: None,
            error: None,
        },
        Some(p) => SourceStatusDto {
            source: normalized.clone(),
            supported: true,
            installed: config_installer::is_plugin_installed(&normalized),
            display_name: p.display_name().to_string(),
            distro: None,
            probe_ok: None,
            error: None,
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

pub fn list_wsl_distros() -> Result<WslDistrosDto, String> {
    wsl_installer::list_distros_detailed()
}

pub fn get_wsl_source_status(source: &str, distro: Option<&str>) -> SourceStatusDto {
    let normalized = normalize_source(source);
    let loader = SourcePluginLoader::new();
    let plugins = loader.load_plugins();
    let plugin = plugins
        .iter()
        .find(|p| p.source_key().eq_ignore_ascii_case(&normalized));

    let resolved_distro = wsl_installer::resolve_distro_name(distro).ok();

    match plugin {
        None => SourceStatusDto {
            source: normalized.clone(),
            supported: false,
            installed: false,
            display_name: normalized,
            distro: resolved_distro,
            probe_ok: Some(true),
            error: None,
        },
        Some(p) => match wsl_installer::is_plugin_installed(&normalized, distro) {
            Ok(installed) => SourceStatusDto {
                source: normalized.clone(),
                supported: true,
                installed,
                display_name: p.display_name().to_string(),
                distro: resolved_distro.or_else(|| distro.map(|d| d.to_string())),
                probe_ok: Some(true),
                error: None,
            },
            Err(message) => SourceStatusDto {
                source: normalized.clone(),
                supported: true,
                installed: false,
                display_name: p.display_name().to_string(),
                distro: resolved_distro.or_else(|| distro.map(|d| d.to_string())),
                probe_ok: Some(false),
                error: Some(message),
            },
        },
    }
}

pub fn install_wsl(source: &str, distro: Option<&str>) -> SourceOperationResultDto {
    run_wsl_source_operation(
        source,
        "installed in WSL",
        distro,
        wsl_installer::install_plugin,
    )
}

pub fn uninstall_wsl(source: &str, distro: Option<&str>) -> SourceOperationResultDto {
    run_wsl_source_operation(
        source,
        "uninstalled from WSL",
        distro,
        wsl_installer::uninstall_plugin,
    )
}

pub fn repair_wsl(source: &str, distro: Option<&str>) -> SourceOperationResultDto {
    run_wsl_source_operation(
        source,
        "repaired in WSL",
        distro,
        wsl_installer::install_plugin,
    )
}

/// 修复所有已安装的数据源（仅 Windows 侧；不含 WSL）
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
            distro: None,
            code: Some("unsupported_source".into()),
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
        distro: None,
        code: if success {
            None
        } else {
            Some("operation_failed".into())
        },
    }
}

fn run_wsl_source_operation(
    source: &str,
    success_verb: &str,
    distro: Option<&str>,
    operation: impl Fn(&str, Option<&str>) -> Result<bool, String>,
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
            distro: distro.map(|d| d.to_string()),
            code: Some("unsupported_source".into()),
        };
    }

    let resolved = wsl_installer::resolve_distro_name(distro).ok();

    match operation(&normalized, distro) {
        Ok(success) => {
            let used_distro = resolved
                .or_else(|| distro.map(|d| d.to_string()))
                .or_else(|| wsl_installer::resolve_distro_name(distro).ok());
            SourceOperationResultDto {
                source: normalized.clone(),
                success,
                installed: wsl_installer::is_plugin_installed(&normalized, distro)
                    .unwrap_or(false),
                message: if success {
                    format!("{normalized} {success_verb}")
                } else {
                    format!("{normalized} WSL operation failed")
                },
                distro: used_distro,
                code: if success {
                    None
                } else {
                    Some("operation_failed".into())
                },
            }
        }
        Err(message) => {
            let code = classify_wsl_error(&message);
            let used_distro = resolved.or_else(|| distro.map(|d| d.to_string()));
            SourceOperationResultDto {
                source: normalized,
                success: false,
                installed: false,
                message,
                distro: used_distro,
                code: Some(code.into()),
            }
        }
    }
}

fn classify_wsl_error(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("unsupported source") {
        "unsupported_source"
    } else if lower.contains("not a user linux")
        || lower.contains("docker") && lower.contains("distro")
    {
        "invalid_distro"
    } else if lower.contains("missing bridge") {
        "missing_bridge"
    } else if lower.contains("hook operation failed") {
        "hook_write_failed"
    } else if lower.contains("wsl")
        || lower.contains("wslpath")
        || lower.contains("no usable wsl")
        || lower.contains("windows runtime")
    {
        "wsl_unavailable"
    } else {
        "operation_failed"
    }
}

fn normalize_source(source: &str) -> String {
    source.trim().to_lowercase()
}
