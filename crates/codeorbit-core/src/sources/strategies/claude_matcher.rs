//! Claude matcher 格式 Hook 策略 — `{hooks: {EventName: [{matcher, hooks: [{type, command, timeout}]}]}}`
//!
//! 外科式增删：安装保留现有 matcher 组、仅追加/替换 CodeOrbit 自己的组；
//! 卸载仅移除 CodeOrbit 的 matcher 组，保留用户自有 hook。

use std::path::Path;

use serde_json::{Map, Value, json};

use super::super::hook_installation_utils::{
    expand_path, get_hook_command, read_json_file, write_json_file,
};
use super::super::plugin_models::HookInstallationSpec;
use super::{HookInstallationStrategy, is_codeorbit_hook_command};

pub struct ClaudeMatcherStrategy;

impl ClaudeMatcherStrategy {
    /// Claude 事件超时：PreToolUse / PermissionRequest / Notification 需 86400s
    fn timeout_for_event(event_name: &str, default_timeout: i32) -> i32 {
        if event_name.eq_ignore_ascii_case("PreToolUse")
            || event_name.eq_ignore_ascii_case("PermissionRequest")
            || event_name.eq_ignore_ascii_case("Notification")
        {
            86400
        } else {
            default_timeout
        }
    }

    fn hooks_object(root: &Value) -> Value {
        root.get("hooks")
            .filter(|h| h.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}))
    }

    /// CodeOrbit 的 matcher 组：空 matcher（匹配全部）+ 单个 command hook
    fn create_codeorbit_group(command: &str, timeout: i32) -> Value {
        json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": command,
                "timeout": timeout,
            }],
        })
    }

    /// 判断某 matcher 组是否为 CodeOrbit（内层任一 hook 命令为 CodeOrbit）
    fn is_codeorbit_group(group: &Value, source_key: &str) -> bool {
        group
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(Value::as_str)
                        .map(|c| is_codeorbit_hook_command(c, source_key))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    /// 保留某事件下非 CodeOrbit 的 matcher 组
    fn keep_non_codeorbit_groups(event_value: &Value, source_key: &str) -> Vec<Value> {
        let mut kept = Vec::new();
        if let Value::Array(groups) = event_value {
            for group in groups {
                if !Self::is_codeorbit_group(group, source_key) {
                    kept.push(group.clone());
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

        // 复制现有事件：保留非 CodeOrbit 组；若该事件在 spec 中则追加 CodeOrbit 组
        if let Value::Object(map) = existing {
            for (event_name, value) in map {
                let mut groups = Self::keep_non_codeorbit_groups(value, source_key);
                if spec
                    .events
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(event_name))
                {
                    groups.push(Self::create_codeorbit_group(
                        &command,
                        Self::timeout_for_event(event_name, spec.timeout_seconds),
                    ));
                }
                if !groups.is_empty() {
                    out.insert(event_name.clone(), Value::Array(groups));
                }
            }
        }

        // 现有中没有的 spec 事件：新增仅含 CodeOrbit 组
        for event in &spec.events {
            let present = existing.is_object() && existing.get(event).is_some();
            if !present {
                out.insert(
                    event.clone(),
                    Value::Array(vec![Self::create_codeorbit_group(
                        &command,
                        Self::timeout_for_event(event, spec.timeout_seconds),
                    )]),
                );
            }
        }

        Value::Object(out)
    }

    fn remove_codeorbit_hooks(hooks_obj: &Value, source_key: &str) -> Value {
        let mut out = Map::new();
        if let Value::Object(map) = hooks_obj {
            for (event_name, value) in map {
                let remaining = Self::keep_non_codeorbit_groups(value, source_key);
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

impl HookInstallationStrategy for ClaudeMatcherStrategy {
    fn install(&self, source_key: &str, spec: &HookInstallationSpec) -> bool {
        let config_path = expand_path(&spec.config_path);
        let root = read_json_file(&config_path);

        let hooks_obj = Self::hooks_object(&root);
        let new_hooks = Self::build_hooks_object(&hooks_obj, source_key, spec);
        let new_root = Self::merge_hooks_into_root(&root, new_hooks);

        write_json_file(&config_path, &new_root).is_ok()
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
        hooks.values().any(|event_value| match event_value {
            Value::Array(groups) => groups
                .iter()
                .any(|group| Self::is_codeorbit_group(group, source_key)),
            _ => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(config_path: &str) -> HookInstallationSpec {
        HookInstallationSpec {
            format: "claude-matcher".to_string(),
            config_path: config_path.to_string(),
            events: vec!["PreToolUse".to_string(), "Stop".to_string()],
            timeout_seconds: 10,
            extra_config: None,
        }
    }

    #[test]
    fn install_preserves_user_hooks_and_uninstall_only_removes_codeorbit() {
        let dir = std::env::temp_dir().join(format!("codeorbit-claude-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("settings.json");
        let config_str = config.to_string_lossy().to_string();

        // 预置用户自有 hook + 其它顶层键
        let preset = json!({
            "model": "claude-sonnet",
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "user-own-hook.sh", "timeout": 5 }] }
                ]
            }
        });
        std::fs::write(&config, serde_json::to_string_pretty(&preset).unwrap()).unwrap();

        let strat = ClaudeMatcherStrategy;
        let spec = spec(&config_str);

        // 安装
        assert!(strat.install("claude", &spec));
        assert!(strat.is_installed("claude", &spec));

        let after_install: Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(after_install["model"], "claude-sonnet"); // 顶层其它键保留
        let pretool = after_install["hooks"]["PreToolUse"].as_array().unwrap();
        assert!(
            pretool.iter().any(|g| g["matcher"] == "Bash"),
            "用户自有 hook 应保留"
        );
        assert!(
            pretool
                .iter()
                .any(|g| ClaudeMatcherStrategy::is_codeorbit_group(g, "claude")),
            "应含 CodeOrbit 组"
        );

        // 卸载
        assert!(strat.uninstall("claude", &spec));
        assert!(!strat.is_installed("claude", &spec));

        let after_uninstall: Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let pretool = after_uninstall["hooks"]["PreToolUse"].as_array().unwrap();
        assert!(
            pretool.iter().any(|g| g["matcher"] == "Bash"),
            "卸载后用户自有 hook 不应被删除"
        );
        assert!(
            !pretool
                .iter()
                .any(|g| ClaudeMatcherStrategy::is_codeorbit_group(g, "claude")),
            "卸载后 CodeOrbit 组应被移除"
        );
        assert_eq!(after_uninstall["model"], "claude-sonnet");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
