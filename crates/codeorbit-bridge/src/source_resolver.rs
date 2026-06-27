//! AI 工具来源推断 — 按优先级从进程祖先链/payload 推断来源

use serde_json::Value;

use codeorbit_core::models::SupportedSource;
use codeorbit_core::sources::{PluginProcessDetector, SourcePluginLoader};

use crate::process_ancestry::{ProcessInfo, process_stem};

/// 可执行文件名（去扩展名）→ 来源映射
fn exe_to_source(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "gemini" => Some("gemini"),
        "cursor" => Some("cursor"),
        "code" => Some("vscode"),
        "copilot" => Some("copilot"),
        "qoder" => Some("qoder"),
        "factory" => Some("droid"),
        "codebuddy" => Some("codebuddy"),
        "opencode" => Some("opencode"),
        "cline" => Some("cline"),
        "node" => Some("node"),
        _ => None,
    }
}

fn is_cursor_cli_indicator(name: &str) -> bool {
    name.eq_ignore_ascii_case("cursor-agent")
}

/// 从进程祖先链推断来源
pub fn infer_source(ancestry: &[ProcessInfo], explicit: Option<&str>, payload: &Value) -> String {
    // 1. 显式来源最高优先级
    if let Some(source) = normalize_source(explicit) {
        return source;
    }

    // 2. 内置来源检测（防止插件覆盖）
    for proc in ancestry {
        let name = process_stem(&proc.name);
        if is_cursor_cli_indicator(&name) {
            return "cursor-cli".to_string();
        }
        if let Some(source) = exe_to_source(&name) {
            if source == "node" {
                if let Some(inferred) = infer_from_node_process(proc) {
                    return inferred.to_string();
                }
                continue;
            }
            return source.to_string();
        }
    }

    // 3. 插件检测规则
    let processes: Vec<(String, Option<String>)> = ancestry
        .iter()
        .map(|p| (p.name.clone(), Some(p.executable_path.clone())))
        .collect();
    let detector = PluginProcessDetector::from_loader(&SourcePluginLoader::new());
    if let Some(plugin_source) = detector.detect_from_process_list(&processes) {
        return plugin_source;
    }

    // 4. payload fallback
    extract_source_from_payload(payload).unwrap_or_else(|| "unknown".to_string())
}

fn infer_from_node_process(proc: &ProcessInfo) -> Option<&'static str> {
    let path = proc.executable_path.to_lowercase();
    if path.contains("opencode") {
        Some("opencode")
    } else if path.contains("cline") {
        Some("cline")
    } else {
        None
    }
}

fn extract_source_from_payload(payload: &Value) -> Option<String> {
    let obj = payload.as_object()?;

    let direct = get_string_field(
        obj,
        &[
            "_source",
            "source",
            "CodeOrbit_SOURCE",
            "CodeOrbit_source",
            "tool_source",
            "toolSource",
        ],
    );
    if let Some(source) = normalize_source(direct.as_deref()) {
        return Some(source);
    }

    if let Some(transcript) = get_string_field(obj, &["transcript_path", "transcriptPath"])
        && transcript.to_lowercase().contains(".claude")
    {
        return Some("claude".to_string());
    }

    for nest_key in ["env", "environment", "payload", "data"] {
        if let Some(nested @ Value::Object(_)) = obj.get(nest_key)
            && let Some(nested_source) = extract_source_from_payload(nested)
        {
            return Some(nested_source);
        }
    }
    None
}

fn get_string_field(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(Value::String(s)) = obj.get(*key) {
            return Some(s.clone());
        }
    }
    None
}

fn normalize_source(source: Option<&str>) -> Option<String> {
    let trimmed = source?.trim();
    if trimmed.is_empty() {
        return None;
    }
    if SupportedSource::is_valid(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn proc(name: &str, exe: &str) -> ProcessInfo {
        ProcessInfo {
            pid: 1,
            parent_pid: 0,
            name: name.to_string(),
            executable_path: exe.to_string(),
            started_at_utc: None,
        }
    }

    #[test]
    fn explicit_source_wins() {
        let source = infer_source(&[], Some("codex"), &json!({}));
        assert_eq!(source, "codex");
    }

    #[test]
    fn exe_map_matches() {
        let ancestry = vec![proc("cursor.exe", "C:/cursor.exe")];
        assert_eq!(infer_source(&ancestry, None, &json!({})), "cursor");
    }

    #[test]
    fn cursor_agent_promoted() {
        let ancestry = vec![proc("cursor-agent", "/usr/bin/cursor-agent")];
        assert_eq!(infer_source(&ancestry, None, &json!({})), "cursor-cli");
    }

    #[test]
    fn node_process_inferred_from_path() {
        let ancestry = vec![proc("node", "/home/u/.opencode/bin/node")];
        assert_eq!(infer_source(&ancestry, None, &json!({})), "opencode");
    }

    #[test]
    fn payload_transcript_path_implies_claude() {
        let payload = json!({ "transcript_path": "/home/u/.claude/projects/x.jsonl" });
        // 无 ancestry / 插件命中时回退到 payload
        let source = infer_source(&[], None, &payload);
        assert_eq!(source, "claude");
    }

    #[test]
    fn unknown_fallback() {
        assert_eq!(infer_source(&[], None, &json!({})), "unknown");
    }
}
