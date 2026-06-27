//! Transcript 增量读取 — 从字节偏移读取 JSONL，解析 Claude/Codex 消息

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::models::ChatMessage;

/// 读取结果：新消息 + 新的字节位置
#[derive(Debug, Clone)]
pub struct TranscriptReadResult {
    pub messages: Vec<ChatMessage>,
    pub position: i64,
}

/// 从 `start_position` 起读取新消息
pub fn read_new_messages(transcript_path: &str, start_position: i64) -> TranscriptReadResult {
    if transcript_path.trim().is_empty() || !Path::new(transcript_path).exists() {
        return empty(start_position);
    }

    let Ok(file) = File::open(transcript_path) else {
        return empty(start_position);
    };
    let len = file.metadata().map(|m| m.len() as i64).unwrap_or(0);
    let safe_start = if start_position > 0 && start_position <= len {
        start_position
    } else {
        0
    };

    let mut reader = BufReader::new(file);
    if reader.seek(SeekFrom::Start(safe_start as u64)).is_err() {
        return empty(start_position);
    }

    let mut messages = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(message) = try_parse_line(line.trim_end_matches(['\n', '\r'])) {
                    messages.push(message);
                }
            }
            Err(_) => break,
        }
    }

    let position = reader.stream_position().map(|p| p as i64).unwrap_or(len);
    TranscriptReadResult { messages, position }
}

fn empty(position: i64) -> TranscriptReadResult {
    TranscriptReadResult {
        messages: Vec::new(),
        position,
    }
}

fn try_parse_line(line: &str) -> Option<ChatMessage> {
    if line.trim().is_empty() {
        return None;
    }
    let root: Value = serde_json::from_str(line).ok()?;

    if let Some(codex) = try_parse_codex_response_item(&root) {
        return Some(codex);
    }

    let role = get_string(&root, &["role"])
        .or_else(|| get_string(&root, &["type"]))
        .or_else(|| get_nested_string(&root, "message", "role"));
    let is_user = role
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case("user"));
    let is_assistant = role
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case("assistant"));
    if !is_user && !is_assistant {
        return None;
    }

    let text = extract_text(&root)?;
    if text.trim().is_empty() {
        return None;
    }

    Some(ChatMessage {
        is_user,
        text,
        timestamp: extract_timestamp(&root).unwrap_or_else(Utc::now),
    })
}

fn try_parse_codex_response_item(root: &Value) -> Option<ChatMessage> {
    if !get_string(root, &["type"])?.eq_ignore_ascii_case("response_item") {
        return None;
    }
    let payload = root.get("payload").filter(|p| p.is_object())?;
    if !get_string(payload, &["type"])?.eq_ignore_ascii_case("message") {
        return None;
    }

    let role = get_string(payload, &["role"]);
    let is_user = role
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case("user"));
    let is_assistant = role
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case("assistant"));
    if !is_user && !is_assistant {
        return None;
    }

    let text = extract_codex_message_text(payload)?;
    if text.trim().is_empty() {
        return None;
    }

    Some(ChatMessage {
        is_user,
        text,
        timestamp: extract_timestamp(root)
            .or_else(|| extract_timestamp(payload))
            .unwrap_or_else(Utc::now),
    })
}

fn extract_text(root: &Value) -> Option<String> {
    if let Some(text) = try_extract_text(root) {
        return Some(text);
    }
    if let Some(message) = root.get("message")
        && let Some(text) = try_extract_text(message)
    {
        return Some(text);
    }
    None
}

fn try_extract_text(element: &Value) -> Option<String> {
    if !element.is_object() {
        return None;
    }
    for key in ["text", "content", "message", "summary"] {
        if let Some(value) = element.get(key)
            && let Some(text) = extract_text_value(value)
            && !text.trim().is_empty()
        {
            return Some(text);
        }
    }
    None
}

fn extract_text_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(extract_text_value)
                .filter(|s| !s.trim().is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(_) => {
            if let Some(Value::String(block_type)) = value.get("type")
                && !block_type.eq_ignore_ascii_case("text")
            {
                return None;
            }
            for key in ["text", "content", "message", "summary"] {
                if let Some(nested) = value.get(key)
                    && let Some(text) = extract_text_value(nested)
                    && !text.trim().is_empty()
                {
                    return Some(text);
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_codex_message_text(payload: &Value) -> Option<String> {
    payload.get("content").and_then(extract_codex_content_text)
}

fn extract_codex_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(extract_codex_content_text)
                .filter(|s| !s.trim().is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(_) => {
            let block_type = get_string(value, &["type"]);
            if let Some(bt) = &block_type
                && !is_codex_visible_text_block(bt)
            {
                return None;
            }
            let candidate_keys: &[&str] = if block_type.is_some() {
                &["text", "input_text", "output_text", "content"]
            } else {
                &["input_text", "output_text", "text"]
            };
            for key in candidate_keys {
                if let Some(nested) = value.get(*key)
                    && let Some(text) = extract_codex_content_text(nested)
                    && !text.trim().is_empty()
                {
                    return Some(text);
                }
            }
            None
        }
        _ => None,
    }
}

fn is_codex_visible_text_block(block_type: &str) -> bool {
    block_type.eq_ignore_ascii_case("input_text")
        || block_type.eq_ignore_ascii_case("output_text")
        || block_type.eq_ignore_ascii_case("text")
}

fn extract_timestamp(root: &Value) -> Option<DateTime<Utc>> {
    let raw = get_string(root, &["timestamp", "created_at", "createdAt"])?;
    if let Ok(dt) = raw.parse::<DateTime<Utc>>() {
        return Some(dt);
    }
    DateTime::parse_from_rfc3339(&raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn get_string(element: &Value, keys: &[&str]) -> Option<String> {
    let obj = element.as_object()?;
    for key in keys {
        if let Some(Value::String(s)) = obj.get(*key) {
            return Some(s.clone());
        }
    }
    None
}

fn get_nested_string(element: &Value, object_key: &str, string_key: &str) -> Option<String> {
    match element.get(object_key)?.get(string_key)? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_claude_and_codex_messages() {
        let dir = std::env::temp_dir().join(format!("codeorbit-tr-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("t.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(f, r#"{{"role":"user","content":"hello"}}"#).unwrap();
        writeln!(
            f,
            r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"hi there"}}]}}}}"#
        )
        .unwrap();
        writeln!(f, r#"{{"type":"other"}}"#).unwrap();
        drop(f);

        let result = read_new_messages(&path.to_string_lossy(), 0);
        assert_eq!(result.messages.len(), 2);
        assert!(result.messages[0].is_user);
        assert_eq!(result.messages[0].text, "hello");
        assert!(!result.messages[1].is_user);
        assert_eq!(result.messages[1].text, "hi there");
        assert!(result.position > 0);

        // 增量读取：从末尾读应无新消息
        let again = read_new_messages(&path.to_string_lossy(), result.position);
        assert_eq!(again.messages.len(), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
