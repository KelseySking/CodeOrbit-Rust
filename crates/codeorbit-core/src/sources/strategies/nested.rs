//! 嵌套对象格式 Hook 策略 — `{hooks: {EventName: [{command, timeout}]}}`（Gemini）

use std::path::Path;

use serde_json::{Map, Value, json};

use super::super::hook_installation_utils::{
    expand_path, get_hook_command, read_json_file, write_json_file,
};
use super::super::plugin_models::HookInstallationSpec;
use super::{HookInstallationStrategy, install_extra_config_toml, is_codeorbit_hook_command};

pub struct NestedHookStrategy;

impl NestedHookStrategy {
    fn hooks_object(root: &Value) -> Value {
        root.get("hooks")
            .filter(|h| h.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}))
    }

    fn create_entry(command: &str, timeout: i32) -> Value {
        json!({ "command": command, "timeout": timeout })
    }

    fn keep_non_codeorbit(event_value: &Value, source_key: &str) -> Vec<Value> {
        let mut kept = Vec::new();
        if let Value::Array(arr) = event_value {
            for hook in arr {
                if let Some(cmd) = hook.get("command").and_then(Value::as_str)
                    && !is_codeorbit_hook_command(cmd, source_key)
                {
                    kept.push(hook.clone());
                }
            }
        }
        kept
    }

    fn build_hooks_object(
        existing: &Value,
        source_key: &str,
        spec: &HookInstallationSpec,
    ) -> Value {
        let mut out = Map::new();
        let command = get_hook_command(source_key);

        if let Value::Object(map) = existing {
            for (event_name, value) in map {
                let mut hooks = Self::keep_non_codeorbit(value, source_key);
                if spec
                    .events
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(event_name))
                {
                    hooks.push(Self::create_entry(&command, spec.timeout_seconds));
                }
                if !hooks.is_empty() {
                    out.insert(event_name.clone(), Value::Array(hooks));
                }
            }
        }

        for event in &spec.events {
            let present = existing.is_object() && existing.get(event).is_some();
            if !present {
                out.insert(
                    event.clone(),
                    Value::Array(vec![Self::create_entry(&command, spec.timeout_seconds)]),
                );
            }
        }

        Value::Object(out)
    }

    fn remove_codeorbit_hooks(hooks_obj: &Value, source_key: &str) -> Value {
        let mut out = Map::new();
        if let Value::Object(map) = hooks_obj {
            for (event_name, value) in map {
                let remaining = Self::keep_non_codeorbit(value, source_key);
                if !remaining.is_empty() {
                    out.insert(event_name.clone(), Value::Array(remaining));
                }
            }
        }
        Value::Object(out)
    }

    fn merge_hooks_into_root(root: &Value, hooks: Value) -> Value {
        let mut out = Map::new();
        if let Value::Object(map) = root {
            for (k, v) in map {
                if k != "hooks" {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
        out.insert("hooks".to_string(), hooks);
        Value::Object(out)
    }
}

impl HookInstallationStrategy for NestedHookStrategy {
    fn install(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        let root = read_json_file(&config_path);

        let hooks_obj = Self::hooks_object(&root);
        let new_hooks = Self::build_hooks_object(&hooks_obj, source_key, spec);
        let new_root = Self::merge_hooks_into_root(&root, new_hooks);

        if write_json_file(&config_path, &new_root).is_err() {
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
            return true;
        }
        let root = read_json_file(&config_path);
        if root.get("hooks").is_none() {
            return true;
        }
        let hooks_obj = Self::hooks_object(&root);
        let cleaned = Self::remove_codeorbit_hooks(&hooks_obj, source_key);
        let new_root = Self::merge_hooks_into_root(&root, cleaned);
        write_json_file(&config_path, &new_root).is_ok()
    }

    fn is_installed(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        if !Path::new(&config_path).exists() {
            return false;
        }
        let root = read_json_file(&config_path);
        let Some(Value::Object(hooks)) = root.get("hooks") else {
            return false;
        };
        hooks.values().any(|event_value| {
            if let Value::Array(arr) = event_value {
                arr.iter().any(|hook| {
                    hook.get("command")
                        .and_then(Value::as_str)
                        .map(|cmd| is_codeorbit_hook_command(cmd, source_key))
                        .unwrap_or(false)
                })
            } else {
                false
            }
        })
    }
}
