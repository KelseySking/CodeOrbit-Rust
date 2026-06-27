//! Codex CLI 格式 Hook 策略 — `{hooks: {EventName: [{hooks: [{type, command, commandWindows?, timeout, statusMessage?}]}]}}`

use std::path::Path;

use serde_json::{Map, Value, json};

use super::super::hook_installation_utils::{
    expand_path, get_bridge_executable_path, get_hook_command, read_json_file, write_json_file,
};
use super::super::plugin_models::HookInstallationSpec;
use super::{HookInstallationStrategy, is_codeorbit_hook_command};

pub struct CodexHookStrategy;

impl CodexHookStrategy {
    /// Codex 超时：PreToolUse / PermissionRequest 需 86400s（可能阻塞等待用户审批）
    fn timeout_for_event(event_name: &str, default_timeout: i32) -> i32 {
        if event_name.eq_ignore_ascii_case("PreToolUse")
            || event_name.eq_ignore_ascii_case("PermissionRequest")
        {
            86400
        } else {
            default_timeout
        }
    }

    /// 判断 Codex 双层嵌套 entry 是否为 CodeOrbit
    fn is_codeorbit_entry(entry: &Value, source_key: &str) -> bool {
        let Some(hooks) = entry.get("hooks").and_then(Value::as_array) else {
            return false;
        };
        hooks.iter().any(|hook| {
            let cmd_match = hook
                .get("command")
                .and_then(Value::as_str)
                .map(|c| is_codeorbit_hook_command(c, source_key))
                .unwrap_or(false);
            let win_match = hook
                .get("commandWindows")
                .and_then(Value::as_str)
                .map(|c| is_codeorbit_hook_command(c, source_key))
                .unwrap_or(false);
            cmd_match || win_match
        })
    }

    /// 创建 Codex hook entry（双层嵌套，含跨平台 command/commandWindows）
    fn create_codex_entry(source_key: &str, timeout_seconds: i32) -> Value {
        let command = get_hook_command(source_key);
        let command_windows = get_codex_windows_command(source_key);
        json!({
            "hooks": [{
                "type": "command",
                "command": command,
                "commandWindows": command_windows,
                "timeout": timeout_seconds,
                "statusMessage": "CodeOrbit context injection",
            }],
        })
    }

    fn hooks_object(root: &Value) -> Value {
        root.get("hooks")
            .filter(|h| h.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}))
    }

    fn build_hooks_object(
        existing: &Value,
        source_key: &str,
        spec: &HookInstallationSpec,
    ) -> Value {
        let mut out = Map::new();

        if let Value::Object(map) = existing {
            for (event_name, value) in map {
                let mut entries: Vec<Value> = Vec::new();
                if let Value::Array(arr) = value {
                    for entry in arr {
                        if !Self::is_codeorbit_entry(entry, source_key) {
                            entries.push(entry.clone());
                        }
                    }
                }
                if spec
                    .events
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(event_name))
                {
                    let timeout = Self::timeout_for_event(event_name, spec.timeout_seconds);
                    entries.push(Self::create_codex_entry(source_key, timeout));
                }
                if !entries.is_empty() {
                    out.insert(event_name.clone(), Value::Array(entries));
                }
            }
        }

        for event in &spec.events {
            let present = existing.is_object() && existing.get(event).is_some();
            if !present {
                let timeout = Self::timeout_for_event(event, spec.timeout_seconds);
                out.insert(
                    event.clone(),
                    Value::Array(vec![Self::create_codex_entry(source_key, timeout)]),
                );
            }
        }

        Value::Object(out)
    }

    fn remove_codeorbit_hooks(hooks_obj: &Value, source_key: &str) -> Value {
        let mut out = Map::new();
        if let Value::Object(map) = hooks_obj {
            for (event_name, value) in map {
                let mut remaining: Vec<Value> = Vec::new();
                if let Value::Array(arr) = value {
                    for entry in arr {
                        if !Self::is_codeorbit_entry(entry, source_key) {
                            remaining.push(entry.clone());
                        }
                    }
                }
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

impl HookInstallationStrategy for CodexHookStrategy {
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
            install_extra_config_toml_codex(extra);
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
                arr.iter()
                    .any(|entry| Self::is_codeorbit_entry(entry, source_key))
            } else {
                false
            }
        })
    }
}

/// Codex 的 Windows 命令：不带引号，含空格时尝试 8.3 短路径
#[cfg(windows)]
fn get_codex_windows_command(source_key: &str) -> String {
    let mut bridge = get_bridge_executable_path();
    if bridge.contains(' ')
        && let Some(short) = try_get_short_path(&bridge)
        && !short.contains(' ')
    {
        bridge = short;
    }
    format!("{bridge} --source {source_key}")
}

#[cfg(not(windows))]
fn get_codex_windows_command(source_key: &str) -> String {
    let bridge = get_bridge_executable_path();
    format!("{bridge} --source {source_key}")
}

#[cfg(windows)]
fn try_get_short_path(long_path: &str) -> Option<String> {
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = OsStr::new(long_path).encode_wide().chain(Some(0)).collect();
    unsafe {
        let len = GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let written = GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), len);
        if written == 0 {
            return None;
        }
        buf.truncate(written as usize);
        Some(OsString::from_wide(&buf).to_string_lossy().into_owned())
    }
}

/// Codex 的 config.toml：启用 hooks（翻转 false→true，迁移 legacy，或插入 [features]）
fn install_extra_config_toml_codex(extra: &super::super::plugin_models::ExtraConfigSpec) {
    use regex::RegexBuilder;
    use std::fs;

    let file_path = expand_path(&extra.file);
    super::super::hook_installation_utils::ensure_directory_exists(&file_path);

    if !file_path.to_lowercase().ends_with(".toml") {
        return;
    }

    let contents = if Path::new(&file_path).exists() {
        fs::read_to_string(&file_path).unwrap_or_default()
    } else {
        String::new()
    };

    let ml = |pat: &str| RegexBuilder::new(pat).multi_line(true).build().unwrap();

    // 已启用
    if ml(r"^\s*hooks\s*=\s*true").is_match(&contents) {
        return;
    }
    // false → true
    if ml(r"^\s*hooks\s*=\s*false").is_match(&contents) {
        let flipped = ml(r"^\s*hooks\s*=\s*false").replace_all(&contents, "hooks = true");
        let _ = fs::write(&file_path, flipped.as_ref());
        return;
    }
    // 迁移 legacy codex_hooks
    if ml(r"^\s*codex_hooks\s*=\s*(true|false)").is_match(&contents) {
        let migrated =
            ml(r"^\s*codex_hooks\s*=\s*(true|false)").replace_all(&contents, "hooks = true");
        let _ = fs::write(&file_path, migrated.as_ref());
        return;
    }

    // 插入到 [features] 段或追加
    let newline = if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut lines: Vec<String> = contents
        .replace("\r\n", "\n")
        .split('\n')
        .map(str::to_string)
        .collect();
    let feature_index = lines.iter().position(|l| l.trim() == "[features]");

    match feature_index {
        Some(idx) => lines.insert(idx + 1, "hooks = true".to_string()),
        None => {
            if lines.last().map(|l| !l.is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
            lines.push("[features]".to_string());
            lines.push("hooks = true".to_string());
        }
    }

    let _ = fs::write(&file_path, lines.join(newline));
}
