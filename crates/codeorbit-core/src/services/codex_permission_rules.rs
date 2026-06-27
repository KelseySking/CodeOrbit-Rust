//! Codex 权限规则 — 将"始终允许"决定写入 Codex 的 prefix_rule 规则文件

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

use super::codex_home::resolve_codex_home;
use crate::models::PermissionRequest;

const NON_PERSISTENT_COMMAND_PREFIXES: &[&str] = &[
    "bash",
    "cmd",
    "node",
    "pwsh",
    "powershell",
    "python",
    "python3",
    "sh",
];

/// 规则文件路径：`<codex_home>/rules/CodeOrbit.rules`
pub fn rules_file_path() -> PathBuf {
    resolve_codex_home().join("rules").join("CodeOrbit.rules")
}

/// 尝试为请求追加一条 allow 规则
pub fn try_append_allow_rule(request: &PermissionRequest) -> bool {
    let Some(pattern) = try_resolve_pattern(request.tool_input.as_ref()) else {
        return false;
    };
    if pattern.is_empty() {
        return false;
    }

    let justification = try_get_string(request.tool_input.as_ref(), &["justification", "reason"])
        .unwrap_or_else(|| "User chose always allow in CodeOrbit.".to_string());
    let line = format_prefix_rule(&pattern, &justification);

    let path = rules_file_path();
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return false;
    }

    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains(&line) {
        return true;
    }

    let newline = if existing.contains("\r\n") || cfg!(windows) {
        "\r\n"
    } else {
        "\n"
    };
    let prefix = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        newline
    };

    std::fs::write(&path, format!("{existing}{prefix}{line}{newline}")).is_ok()
}

fn format_prefix_rule(pattern: &[String], justification: &str) -> String {
    let escaped: Vec<String> = pattern
        .iter()
        .map(|item| format!("\"{}\"", escape_rule_string(item)))
        .collect();
    format!(
        "prefix_rule(pattern = [{}], decision = \"allow\", justification = \"{}\")",
        escaped.join(", "),
        escape_rule_string(justification)
    )
}

fn try_resolve_pattern(input: Option<&HashMap<String, Value>>) -> Option<Vec<String>> {
    let input = input?;

    if let Some(prefix_rule) = get_value_ignore_case(input, &["prefix_rule", "prefixRule"])
        && let Some(pattern) = try_read_prefix_rule(prefix_rule)
    {
        return Some(pattern);
    }

    if let Some(command) = try_get_string(Some(input), &["command"]) {
        return try_parse_shell_prefix(&command);
    }

    None
}

fn try_read_prefix_rule(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::Array(_) | Value::Object(_) => try_read_prefix_rule_element(value),
        Value::String(s) => try_read_prefix_rule_string(s),
        _ => None,
    }
}

fn try_read_prefix_rule_string(text: &str) -> Option<Vec<String>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        return serde_json::from_str::<Value>(trimmed)
            .ok()
            .and_then(|el| try_read_prefix_rule_element(&el));
    }
    Some(vec![trimmed.to_string()])
}

fn try_read_prefix_rule_element(element: &Value) -> Option<Vec<String>> {
    if let Value::Object(_) = element
        && let Some(pattern) = get_property_ignore_case(element, "pattern")
    {
        return try_read_prefix_rule_element(pattern);
    }

    let Value::Array(items) = element else {
        return None;
    };

    let values: Vec<String> = items
        .iter()
        .map(|item| match item {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .filter(|s| !s.trim().is_empty())
        .collect();

    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn try_parse_shell_prefix(command: &str) -> Option<Vec<String>> {
    let tokens = tokenize_shell_prefix(command);
    if tokens.len() < 2 {
        return None;
    }
    if NON_PERSISTENT_COMMAND_PREFIXES
        .iter()
        .any(|p| p.eq_ignore_ascii_case(&tokens[0]))
    {
        return None;
    }

    let count = if tokens[0].eq_ignore_ascii_case("npm")
        && tokens.len() >= 3
        && tokens[1].eq_ignore_ascii_case("run")
    {
        3
    } else {
        2
    };

    Some(tokens.into_iter().take(count).collect())
}

fn tokenize_shell_prefix(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = '\0';

    for ch in command.chars() {
        if quote == '\0' {
            if ch.is_whitespace() {
                flush(&mut tokens, &mut current);
                if tokens.len() >= 3 {
                    break;
                }
                continue;
            }
            if matches!(ch, '|' | ';' | '<' | '>' | '&') {
                break;
            }
            if ch == '"' || ch == '\'' {
                quote = ch;
                continue;
            }
            current.push(ch);
            continue;
        }

        if ch == quote {
            quote = '\0';
            continue;
        }
        current.push(ch);
    }

    flush(&mut tokens, &mut current);
    tokens
}

fn flush(tokens: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    tokens.push(std::mem::take(current));
}

fn try_get_string(input: Option<&HashMap<String, Value>>, names: &[&str]) -> Option<String> {
    let value = get_value_ignore_case(input?, names)?;
    match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
        _ => None,
    }
}

fn get_value_ignore_case<'a>(
    input: &'a HashMap<String, Value>,
    names: &[&str],
) -> Option<&'a Value> {
    input
        .iter()
        .find(|(k, _)| names.iter().any(|n| n.eq_ignore_ascii_case(k)))
        .map(|(_, v)| v)
}

fn get_property_ignore_case<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value
        .as_object()?
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v)
}

fn escape_rule_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shell_prefix_two_tokens() {
        assert_eq!(
            try_parse_shell_prefix("git status --porcelain"),
            Some(vec!["git".to_string(), "status".to_string()])
        );
    }

    #[test]
    fn npm_run_takes_three_tokens() {
        assert_eq!(
            try_parse_shell_prefix("npm run build --watch"),
            Some(vec![
                "npm".to_string(),
                "run".to_string(),
                "build".to_string()
            ])
        );
    }

    #[test]
    fn rejects_non_persistent_prefixes() {
        assert_eq!(try_parse_shell_prefix("bash -c 'rm -rf /'"), None);
        assert_eq!(try_parse_shell_prefix("python script.py"), None);
    }

    #[test]
    fn format_rule_escapes() {
        let line = format_prefix_rule(&["git".to_string(), "status".to_string()], "because");
        assert!(line.contains(r#"pattern = ["git", "status"]"#));
        assert!(line.contains(r#"decision = "allow""#));
    }
}
