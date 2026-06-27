//! 扁平数组格式 Hook 策略 — `[{event, command, timeout}]`（Cursor, Trae）

use std::path::Path;

use serde_json::{Value, json};

use super::super::hook_installation_utils::{expand_path, read_json_file, write_json_file};
use super::super::plugin_models::HookInstallationSpec;
use super::{HookInstallationStrategy, install_extra_config_toml, is_codeorbit_hook_command};

pub struct FlatHookStrategy;

impl FlatHookStrategy {
    /// 保留非 CodeOrbit 的 hook 项
    fn retain_non_codeorbit(existing: &Value, source_key: &str) -> Vec<Value> {
        let mut kept = Vec::new();
        if let Value::Array(arr) = existing {
            for item in arr {
                if let Some(cmd) = item.get("command").and_then(Value::as_str)
                    && !is_codeorbit_hook_command(cmd, source_key)
                {
                    kept.push(item.clone());
                }
            }
        }
        kept
    }
}

impl HookInstallationStrategy for FlatHookStrategy {
    fn install(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        let existing = read_json_file(&config_path);

        let mut hooks = Self::retain_non_codeorbit(&existing, source_key);
        let command = super::super::hook_installation_utils::get_hook_command(source_key);
        for event in &spec.events {
            hooks.push(json!({
                "event": event,
                "command": command,
                "timeout": spec.timeout_seconds,
            }));
        }

        if write_json_file(&config_path, &Value::Array(hooks)).is_err() {
            return false;
        }

        if let Some(extra) = &spec.extra_config {
            install_extra_config_toml(extra);
        }
        true
    }

    fn uninstall(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        if !Path::new(&config_path).exists() {
            return true; // 已卸载
        }
        let existing = read_json_file(&config_path);
        let remaining = Self::retain_non_codeorbit(&existing, source_key);
        write_json_file(&config_path, &Value::Array(remaining)).is_ok()
    }

    fn is_installed(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        if !Path::new(&config_path).exists() {
            return false;
        }
        let existing = read_json_file(&config_path);
        let Value::Array(arr) = existing else {
            return false;
        };
        arr.iter().any(|item| {
            item.get("command")
                .and_then(Value::as_str)
                .map(|cmd| is_codeorbit_hook_command(cmd, source_key))
                .unwrap_or(false)
        })
    }
}
