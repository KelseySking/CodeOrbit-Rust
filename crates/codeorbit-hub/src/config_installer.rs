//! ConfigInstaller — 插件 hook 安装/卸载/检测 + 运行时资源路径
//!
//! 插件安装委托给 p5 的 HookStrategy（经 SourcePluginLoader + HookStrategyFactory），
//! 并通过 `set_bridge_executable_path` 将运行时 bridge 路径注入到生成的 hook 命令。

use std::path::PathBuf;

use codeorbit_core::sources::hook_installation_utils::set_bridge_executable_path;
use codeorbit_core::sources::{SourcePluginLoader, hook_strategy_factory};

const HOOK_SCRIPT_NAME: &str = "CodeOrbit-hook.ps1";

fn bridge_exe_name() -> &'static str {
    if cfg!(windows) {
        "codeorbit-bridge.exe"
    } else {
        "codeorbit-bridge"
    }
}

/// RuntimeHost 所在目录：优先环境变量 `CodeOrbit_RUNTIME_DIR`，否则 exe 同级目录
pub fn runtime_directory() -> PathBuf {
    if let Ok(dir) = std::env::var("CodeOrbit_RUNTIME_DIR")
        && !dir.is_empty()
    {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Bridge 可执行文件路径（与 RuntimeHost 同目录）
pub fn runtime_bridge_exe_path() -> PathBuf {
    runtime_directory().join(bridge_exe_name())
}

/// Hook 脚本路径
pub fn runtime_hook_script_path() -> PathBuf {
    runtime_directory().join(HOOK_SCRIPT_NAME)
}

/// 运行时资源是否已就位（bridge 可执行文件存在）
pub fn are_runtime_assets_installed() -> bool {
    runtime_bridge_exe_path().exists()
}

/// 修复运行时资源：确保 bridge 存在并重装已安装的插件 hook（更新路径）
pub fn repair_runtime_assets() -> bool {
    if !runtime_bridge_exe_path().exists() {
        return false;
    }
    repair_installed_hook_configurations();
    true
}

/// 重装所有已安装的插件 hook（启动时更新 hook 路径指向当前 RuntimeHost）
pub fn repair_installed_hook_configurations() -> bool {
    let loader = SourcePluginLoader::new();
    let mut all_ok = true;
    for plugin in loader.load_plugins() {
        let key = source_key(&plugin);
        if is_plugin_installed(&key) {
            all_ok &= install_plugin(&key);
        }
    }
    all_ok
}

/// 安装指定插件的 hook 配置
pub fn install_plugin(source_key: &str) -> bool {
    let Some(spec) = find_hook_spec(source_key) else {
        return false;
    };
    // bridge 必须存在
    if !runtime_bridge_exe_path().exists() {
        return false;
    }
    set_bridge_executable_path(runtime_bridge_exe_path().to_string_lossy().into_owned());

    match hook_strategy_factory::create(&spec.format) {
        Some(strategy) => strategy.install(source_key, &spec),
        None => false,
    }
}

/// 卸载指定插件的 hook 配置
pub fn uninstall_plugin(source_key: &str) -> bool {
    let Some(spec) = find_hook_spec(source_key) else {
        // 无 hook spec → 无需卸载（视为成功）；插件不存在也走这里返回 true 不合适，
        // 但 find_hook_spec 仅在插件存在且有 spec 时返回 Some，故区分见下。
        return !plugin_exists(source_key);
    };
    match hook_strategy_factory::create(&spec.format) {
        Some(strategy) => strategy.uninstall(source_key, &spec),
        None => false,
    }
}

/// 检查插件 hook 是否已安装
pub fn is_plugin_installed(source_key: &str) -> bool {
    let Some(spec) = find_hook_spec(source_key) else {
        return false;
    };
    match hook_strategy_factory::create(&spec.format) {
        Some(strategy) => strategy.is_installed(source_key, &spec),
        None => false,
    }
}

fn find_hook_spec(
    source_key: &str,
) -> Option<codeorbit_core::sources::plugin_models::HookInstallationSpec> {
    use codeorbit_core::sources::adapter_trait::SourceAdapter;
    let loader = SourcePluginLoader::new();
    loader
        .load_plugins()
        .iter()
        .find(|p| p.source_key().eq_ignore_ascii_case(source_key))
        .and_then(|p| p.hook_installation_spec().cloned())
}

fn plugin_exists(source_key: &str) -> bool {
    use codeorbit_core::sources::adapter_trait::SourceAdapter;
    SourcePluginLoader::new()
        .load_plugins()
        .iter()
        .any(|p| p.source_key().eq_ignore_ascii_case(source_key))
}

fn source_key(plugin: &codeorbit_core::sources::PluginSourceAdapter) -> String {
    use codeorbit_core::sources::adapter_trait::SourceAdapter;
    plugin.source_key().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_paths_use_env_override() {
        let dir = std::env::temp_dir().join(format!("codeorbit-rt-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // SAFETY: 单线程测试设置进程环境变量
        unsafe {
            std::env::set_var("CodeOrbit_RUNTIME_DIR", &dir);
        }

        assert_eq!(runtime_directory(), dir);
        assert!(runtime_hook_script_path().ends_with("CodeOrbit-hook.ps1"));
        assert!(!are_runtime_assets_installed());

        // 放置一个假 bridge 文件
        std::fs::write(runtime_bridge_exe_path(), b"fake").unwrap();
        assert!(are_runtime_assets_installed());

        unsafe {
            std::env::remove_var("CodeOrbit_RUNTIME_DIR");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
