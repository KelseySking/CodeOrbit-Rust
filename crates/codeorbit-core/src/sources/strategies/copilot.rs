//! GitHub Copilot 格式 Hook 策略 — `{version, hooks: [{event, command, timeout}]}`

use std::path::Path;

use serde_json::{Map, Value, json};

use super::super::hook_installation_utils::{
    ensure_directory_exists, expand_path, get_hook_command,
};
use super::super::plugin_models::HookInstallationSpec;
use super::{HookInstallationStrategy, is_codeorbit_hook_command};

pub struct CopilotHookStrategy;

impl CopilotHookStrategy {
    fn read_object(config_path: &str) -> Map<String, Value> {
        if !Path::new(config_path).exists() {
            return Map::new();
        }
        match std::fs::read_to_string(config_path) {
            Ok(content) => match serde_json::from_str::<Value>(&content) {
                Ok(Value::Object(map)) => map,
                _ => Map::new(),
            },
            Err(_) => Map::new(),
        }
    }

    fn write_object(config_path: &str, root: &Map<String, Value>) -> bool {
        ensure_directory_exists(config_path);
        let text = serde_json::to_string_pretty(&Value::Object(root.clone()))
            .unwrap_or_else(|_| "{}".to_string());
        std::fs::write(config_path, text).is_ok()
    }

    fn is_codeorbit_hook(hook: &Value, source_key: &str) -> bool {
        hook.get("command")
            .and_then(Value::as_str)
            .map(|cmd| is_codeorbit_hook_command(cmd, source_key))
            .unwrap_or(false)
    }
}

impl HookInstallationStrategy for CopilotHookStrategy {
    fn install(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        let mut root = Self::read_object(&config_path);
        root.entry("version".to_string()).or_insert(json!(1));

        let mut merged: Vec<Value> = Vec::new();
        if let Some(Value::Array(hooks)) = root.get("hooks") {
            for hook in hooks {
                if !Self::is_codeorbit_hook(hook, source_key) {
                    merged.push(hook.clone());
                }
            }
        }

        let command = get_hook_command(source_key);
        for event in &spec.events {
            merged.push(json!({
                "event": event,
                "command": command,
                "timeout": spec.timeout_seconds,
            }));
        }

        root.insert("hooks".to_string(), Value::Array(merged));
        Self::write_object(&config_path, &root)
    }

    fn uninstall(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        if !Path::new(&config_path).exists() {
            return true;
        }
        let mut root = Self::read_object(&config_path);
        let Some(Value::Array(hooks)) = root.get("hooks") else {
            return true;
        };

        let remaining: Vec<Value> = hooks
            .iter()
            .filter(|h| !Self::is_codeorbit_hook(h, source_key))
            .cloned()
            .collect();

        root.insert("hooks".to_string(), Value::Array(remaining));
        Self::write_object(&config_path, &root)
    }

    fn is_installed(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        if !Path::new(&config_path).exists() {
            return false;
        }
        let root = Self::read_object(&config_path);
        let Some(Value::Array(hooks)) = root.get("hooks") else {
            return false;
        };
        hooks.iter().any(|h| Self::is_codeorbit_hook(h, source_key))
    }
}
