//! 插件 JSON 解析与校验

use std::collections::HashMap;

use regex::Regex;
use serde_json::Value;

use super::plugin_models::{
    DetectionRule, ExtraConfigSpec, HookInstallationSpec, PermissionResponseStyle, PluginMetadata,
    PluginValidationError,
};
use super::plugin_validator;

/// 解析错误：消息 + 校验错误分类
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub kind: PluginValidationError,
}

impl ParseError {
    fn new(message: impl Into<String>, kind: PluginValidationError) -> Self {
        Self {
            message: message.into(),
            kind,
        }
    }
}

fn source_key_re() -> Regex {
    Regex::new(r"^[a-z0-9][a-z0-9-]{0,62}[a-z0-9]$").unwrap()
}

/// 解析插件 JSON，校验后返回元数据
pub fn parse(
    json_content: &str,
    existing_source_keys: &[String],
) -> Result<PluginMetadata, ParseError> {
    let root: Value = serde_json::from_str(json_content).map_err(|e| {
        ParseError::new(
            format!("Invalid JSON: {e}"),
            PluginValidationError::InvalidJson,
        )
    })?;

    // schema_version（支持 1.0 与 2.0）
    let version = root
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ParseError::new(
                "Missing 'schema_version'.",
                PluginValidationError::InvalidSchemaVersion,
            )
        })?;
    if version != "1.0" && version != "2.0" {
        return Err(ParseError::new(
            format!("Invalid 'schema_version'. Expected '1.0' or '2.0', got: '{version}'"),
            PluginValidationError::InvalidSchemaVersion,
        ));
    }

    // source 对象
    let source = root.get("source").ok_or_else(|| {
        ParseError::new(
            "Missing required 'source' object.",
            PluginValidationError::MissingRequiredField,
        )
    })?;

    // source.key
    let source_key = source
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ParseError::new(
                "Missing required 'source.key'.",
                PluginValidationError::MissingRequiredField,
            )
        })?
        .trim()
        .to_string();
    if source_key.is_empty() {
        return Err(ParseError::new(
            "'source.key' cannot be empty.",
            PluginValidationError::InvalidSourceKey,
        ));
    }
    if !source_key_re().is_match(&source_key) {
        return Err(ParseError::new(
            format!(
                "'source.key' must match pattern: lowercase alphanumeric with hyphens, 2-64 chars. Got: '{source_key}'"
            ),
            PluginValidationError::InvalidSourceKey,
        ));
    }
    if existing_source_keys
        .iter()
        .any(|k| k.eq_ignore_ascii_case(&source_key))
    {
        return Err(ParseError::new(
            format!(
                "Source key '{source_key}' already exists (duplicate or conflicts with built-in)."
            ),
            PluginValidationError::DuplicateSourceKey,
        ));
    }

    // source.display_name (1-100)
    let display_name = source
        .get("display_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if display_name.is_empty() || display_name.chars().count() > 100 {
        return Err(ParseError::new(
            "'source.display_name' must be 1-100 characters.",
            PluginValidationError::MissingRequiredField,
        ));
    }

    // source.icon_name (1-64)
    let icon_name = source
        .get("icon_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if icon_name.is_empty() || icon_name.chars().count() > 64 {
        return Err(ParseError::new(
            "'source.icon_name' must be 1-64 characters.",
            PluginValidationError::MissingRequiredField,
        ));
    }

    // source.permission_response_style
    let style_string = source
        .get("permission_response_style")
        .and_then(Value::as_str);
    let permission_style = parse_permission_style(style_string).ok_or_else(|| {
        ParseError::new(
            format!(
                "Invalid 'source.permission_response_style'. Expected 'claude-style' or 'codex', got: '{}'",
                style_string.unwrap_or("")
            ),
            PluginValidationError::InvalidPermissionStyle,
        )
    })?;

    // event_mappings（可选）
    let mut event_mappings: HashMap<String, String> = HashMap::new();
    if let Some(map) = root.get("event_mappings").and_then(Value::as_object) {
        for (name, value) in map {
            let target = value.as_str().map(str::trim).unwrap_or("");
            if target.is_empty() {
                continue;
            }
            if !is_valid_event_name(target) {
                return Err(ParseError::new(
                    format!(
                        "Invalid event mapping target: '{target}'. Must be a standard event name."
                    ),
                    PluginValidationError::InvalidEventMapping,
                ));
            }
            event_mappings.insert(name.clone(), target.to_string());
        }
    }

    // detection（可选，schema 2.0）
    let detection = match root.get("detection") {
        Some(el) => Some(parse_detection(&source_key, el)?),
        None => None,
    };

    // hook_installation（可选，schema 2.0）
    let hook_installation = match root.get("hook_installation") {
        Some(el) => Some(parse_hook_installation(el)?),
        None => None,
    };

    Ok(PluginMetadata {
        source_key,
        display_name: display_name.to_string(),
        icon_name: icon_name.to_string(),
        permission_response_style: permission_style,
        event_mappings,
        detection,
        hook_installation,
    })
}

fn parse_permission_style(style: Option<&str>) -> Option<PermissionResponseStyle> {
    match style.map(str::trim).unwrap_or("").to_lowercase().as_str() {
        "claude-style" => Some(PermissionResponseStyle::ClaudeStyle),
        "codex" => Some(PermissionResponseStyle::Codex),
        _ => None,
    }
}

fn is_valid_event_name(event_name: &str) -> bool {
    plugin_validator::STANDARD_EVENTS
        .iter()
        .any(|e| e.eq_ignore_ascii_case(event_name))
}

fn parse_detection(source_key: &str, el: &Value) -> Result<DetectionRule, ParseError> {
    if !el.is_object() {
        return Err(ParseError::new(
            "'detection' must be an object",
            PluginValidationError::InvalidJson,
        ));
    }

    let process_names = string_array(el.get("process_names"));

    let mut env_var_hints: HashMap<String, String> = HashMap::new();
    if let Some(obj) = el.get("env_var_hints").and_then(Value::as_object) {
        for (k, v) in obj {
            if let Some(pattern) = v.as_str().map(str::trim)
                && !pattern.is_empty()
            {
                env_var_hints.insert(k.clone(), pattern.to_string());
            }
        }
    }

    let path_patterns = string_array(el.get("path_patterns"));

    let priority = el
        .get("priority")
        .and_then(Value::as_i64)
        .map(|n| n as i32)
        .unwrap_or(100);

    plugin_validator::validate_detection(&process_names, &env_var_hints, &path_patterns, priority)
        .map_err(|e| {
            ParseError::new(
                format!("Detection validation failed: {e}"),
                PluginValidationError::InvalidJson,
            )
        })?;

    Ok(DetectionRule {
        source_key: source_key.to_string(),
        process_names,
        env_var_hints,
        path_patterns,
        priority,
    })
}

fn parse_hook_installation(el: &Value) -> Result<HookInstallationSpec, ParseError> {
    if !el.is_object() {
        return Err(ParseError::new(
            "'hook_installation' must be an object",
            PluginValidationError::InvalidJson,
        ));
    }

    let format = el
        .get("format")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ParseError::new(
                "Missing required 'hook_installation.format'",
                PluginValidationError::InvalidJson,
            )
        })?
        .to_string();

    let config_path = el
        .get("config_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ParseError::new(
                "Missing required 'hook_installation.config_path'",
                PluginValidationError::InvalidJson,
            )
        })?
        .to_string();

    let events_val = el.get("events");
    if !events_val.map(Value::is_array).unwrap_or(false) {
        return Err(ParseError::new(
            "Missing or invalid 'hook_installation.events' (must be array)",
            PluginValidationError::InvalidJson,
        ));
    }
    let events = string_array(events_val);

    let timeout_seconds = el
        .get("timeout_seconds")
        .and_then(Value::as_i64)
        .map(|n| n as i32)
        .unwrap_or(10);

    let extra_config = match el.get("extra_config") {
        Some(extra) if extra.is_object() => Some(parse_extra_config(extra)?),
        _ => None,
    };

    plugin_validator::validate_hook_installation(&format, &config_path, &events, timeout_seconds)
        .map_err(|e| {
        ParseError::new(
            format!("Hook installation validation failed: {e}"),
            PluginValidationError::InvalidJson,
        )
    })?;

    Ok(HookInstallationSpec {
        format,
        config_path,
        events,
        timeout_seconds,
        extra_config,
    })
}

fn parse_extra_config(el: &Value) -> Result<ExtraConfigSpec, ParseError> {
    let file = required_trimmed(el, "file", "extra_config.file")?;
    let key = required_trimmed(el, "key", "extra_config.key")?;
    let value = required_trimmed(el, "value", "extra_config.value")?;
    let section = el
        .get("section")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string());

    Ok(ExtraConfigSpec {
        file,
        section,
        key,
        value,
    })
}

fn required_trimmed(el: &Value, field: &str, label: &str) -> Result<String, ParseError> {
    el.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            ParseError::new(
                format!("Missing required '{label}'"),
                PluginValidationError::InvalidJson,
            )
        })
}

/// 从 JSON 数组提取去除空白后的非空字符串列表
fn string_array(val: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = val else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}
