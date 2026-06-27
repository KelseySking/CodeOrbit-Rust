//! Transcript 路径解析 — 提取路径/工作目录、定位 Codex 会话、解码 Claude 项目段

use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::codex_home::resolve_codex_home;

/// 从事件载荷提取 transcript 路径（支持嵌套）
pub fn extract_transcript_path(element: Option<&Value>) -> Option<String> {
    let obj = element?.as_object()?;

    if let Some(direct) = get_string_field(element?, &["transcript_path", "transcriptPath"]) {
        let trimmed = direct.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    for nest_key in ["payload", "data", "context", "workspace"] {
        if let Some(nested @ Value::Object(_)) = obj.get(nest_key)
            && let Some(path) = extract_transcript_path(Some(nested))
            && !path.trim().is_empty()
        {
            return Some(path);
        }
    }
    None
}

/// 从事件载荷提取工作目录（支持嵌套，回退到 transcript 路径推导）
pub fn extract_working_directory(element: Option<&Value>) -> Option<String> {
    let obj = element?.as_object()?;

    let direct = get_string_field(
        element?,
        &[
            "cwd",
            "current_dir",
            "currentDir",
            "working_directory",
            "workingDirectory",
            "workspace",
            "workspaceFolder",
            "workspace_folder",
            "workspacePath",
            "workspace_path",
        ],
    );
    if let Some(value) = direct
        && let Some(dir) = normalize_directory_candidate(&value)
    {
        return Some(dir);
    }

    if let Some(transcript) = extract_transcript_path(element)
        && !transcript.trim().is_empty()
    {
        let project = get_project_directory_from_transcript_path(&transcript);
        if let Some(dir) = normalize_directory_candidate(&project) {
            return Some(dir);
        }
    }

    for nest_key in ["payload", "data", "context", "workspace"] {
        if let Some(nested @ Value::Object(_)) = obj.get(nest_key)
            && let Some(dir) = extract_working_directory(Some(nested))
            && !dir.trim().is_empty()
        {
            return Some(dir);
        }
    }
    None
}

/// 按 session id 在 Codex sessions 目录定位最新的 transcript 文件
pub fn try_resolve_codex_transcript_path(session_id: Option<&str>) -> Option<String> {
    let session_id = session_id?.trim();
    if session_id.is_empty() || has_invalid_filename_chars(session_id) {
        return None;
    }

    let sessions_dir = resolve_codex_home().join("sessions");
    if !sessions_dir.exists() {
        return None;
    }

    let mut best: Option<(PathBuf, DateTime<Utc>)> = None;
    for candidate in walk_files(&sessions_dir) {
        if !is_codex_transcript_file_for_session(&candidate, session_id) {
            continue;
        }
        let timestamp = last_write_time_utc(&candidate);
        match &best {
            Some((_, best_ts)) if timestamp < *best_ts => {}
            _ => best = Some((candidate, timestamp)),
        }
    }

    best.map(|(path, _)| path.to_string_lossy().into_owned())
}

/// 从 transcript 路径推导项目目录（解码 Claude `.claude/projects/<encoded>` 段）
pub fn get_project_directory_from_transcript_path(transcript_path: &str) -> String {
    let directory = Path::new(transcript_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| transcript_path.to_string());

    let segments = split_path_segments(&directory);
    let claude_index = segments
        .iter()
        .rposition(|s| s.eq_ignore_ascii_case(".claude"));

    if let Some(idx) = claude_index {
        if idx + 2 < segments.len() && segments[idx + 1].eq_ignore_ascii_case("projects") {
            return decode_claude_transcript_project_segment(&segments[idx + 2])
                .unwrap_or_else(|| directory.clone());
        }
        if idx == segments.len() - 1 {
            return Path::new(&directory)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| directory.clone());
        }
    }

    directory
}

/// 提取路径的最后一段作为项目名
pub fn extract_project_name(path: Option<&str>) -> Option<String> {
    let trimmed = path?.trim().trim_end_matches(['/', '\\', MAIN_SEPARATOR]);
    if trimmed.is_empty() {
        return None;
    }
    split_path_segments(trimmed).into_iter().last()
}

/// 按 `/` 和 `\` 切分路径为非空段
pub fn split_path_segments(path: &str) -> Vec<String> {
    path.split(['/', '\\', MAIN_SEPARATOR])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_directory_candidate(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_codex_transcript_file_for_session(path: &Path, session_id: &str) -> bool {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| !e.eq_ignore_ascii_case("jsonl"))
        .unwrap_or(true)
    {
        return false;
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|name| name.to_lowercase().ends_with(&session_id.to_lowercase()))
        .unwrap_or(false)
}

fn last_write_time_utc(path: &Path) -> DateTime<Utc> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
}

fn decode_claude_transcript_project_segment(segment: &str) -> Option<String> {
    let normalized = segment.trim();
    if normalized.is_empty() {
        return None;
    }
    let sep = MAIN_SEPARATOR;
    let chars: Vec<char> = normalized.chars().collect();

    // 形如 "C--Users-foo" → "C:\Users\foo"
    if chars.len() >= 3 && chars[0].is_ascii_alphabetic() && chars[1] == '-' && chars[2] == '-' {
        let drive = chars[0].to_ascii_uppercase();
        let rest: String = normalized[3..].replace('-', &sep.to_string());
        return Some(format!("{drive}:{sep}{rest}"));
    }
    // 形如 "-C--Users-foo"
    if chars.len() >= 4 && chars[0] == '-' && chars[1].is_ascii_alphabetic() && chars[2] == '-' {
        let drive = chars[1].to_ascii_uppercase();
        let rest: String = normalized[3..].replace('-', &sep.to_string());
        return Some(format!("{drive}:{sep}{rest}"));
    }
    if let Some(stripped) = normalized.strip_prefix('-') {
        return Some(format!("{sep}{}", stripped.replace('-', &sep.to_string())));
    }
    Some(normalized.replace('-', &sep.to_string()))
}

fn get_string_field(json: &Value, keys: &[&str]) -> Option<String> {
    let obj = json.as_object()?;
    for key in keys {
        if let Some(Value::String(s)) = obj.get(*key) {
            return Some(s.clone());
        }
    }
    None
}

fn has_invalid_filename_chars(name: &str) -> bool {
    name.chars().any(|c| {
        matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control()
    })
}

/// 递归收集目录下所有文件
fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk_files(&path));
        } else if path.is_file() {
            files.push(path);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_nested_transcript_path() {
        let payload = json!({ "payload": { "transcript_path": "/tmp/a.jsonl" } });
        assert_eq!(
            extract_transcript_path(Some(&payload)).as_deref(),
            Some("/tmp/a.jsonl")
        );
    }

    #[test]
    fn extracts_direct_working_directory() {
        let payload = json!({ "cwd": "/home/user/proj" });
        assert_eq!(
            extract_working_directory(Some(&payload)).as_deref(),
            Some("/home/user/proj")
        );
    }

    #[test]
    fn decodes_claude_project_segment() {
        let decoded = decode_claude_transcript_project_segment("C--Users-foo-bar").unwrap();
        assert!(decoded.starts_with("C:"));
        assert!(decoded.contains("Users"));
    }

    #[test]
    fn extracts_project_name_last_segment() {
        assert_eq!(
            extract_project_name(Some("/home/user/my-proj/")).as_deref(),
            Some("my-proj")
        );
    }
}
