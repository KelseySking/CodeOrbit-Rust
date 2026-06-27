//! Bridge 主流程 — stdin → 解析 → 富化 → 序列化 → 分类 → (IPC) → stdout
//!
//! Bridge 是宿主 CLI 的 hook 进程，必须快速返回且绝不阻断宿主工具链。
//! IPC 客户端（Task 3.3）在 `send_to_hub` 接入。

use std::time::Duration;

use serde_json::{Map, Value};
use tokio::io::AsyncReadExt;

use crate::tracked_process_resolver::TrackedProcess;
use crate::{
    bridge_client, environment_collector, event_classifier, field_normalizer, payload_serializer,
    process_ancestry, source_resolver, tracked_process_resolver,
};

const STDIN_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ANCESTRY_DEPTH: usize = 12;

/// 运行 Bridge，返回进程退出码
pub async fn run(args: Vec<String>) -> i32 {
    // 1. 读取 stdin（5 秒超时）
    let mut input = String::new();
    let read =
        tokio::time::timeout(STDIN_TIMEOUT, tokio::io::stdin().read_to_string(&mut input)).await;
    match read {
        Err(_) => return 1,     // 超时
        Ok(Err(_)) => return 1, // 读取失败
        Ok(Ok(_)) => {}
    }

    // 空输入：无事可做
    if input.trim().is_empty() {
        return 0;
    }

    // 2. 解析原始 JSON（必须是对象）
    let Ok(root) = serde_json::from_str::<Value>(&input) else {
        return 1;
    };
    let Some(obj) = root.as_object() else {
        return 1;
    };

    // 3. 进程检测与来源推断
    let explicit_source = parse_args_for_source(&args);
    let parent_pid = process_ancestry::get_parent_pid();
    let ancestry = process_ancestry::build_ancestry(parent_pid, MAX_ANCESTRY_DEPTH);
    let terminal_env = environment_collector::collect();
    let source = source_resolver::infer_source(&ancestry, explicit_source.as_deref(), &root);
    let tracked = tracked_process_resolver::resolve(&ancestry, parent_pid, &terminal_env);

    // 4. 富化 payload
    let payload = enrich_payload(obj, &source, parent_pid, &tracked, &terminal_env);

    // 5. 序列化 + 事件分类
    let enriched_json = payload_serializer::serialize(&payload);
    let blocking = event_classifier::is_blocking_event(&payload);

    // 6. 通过 IPC 发送到 Hub。连接失败/超时 → 静默返回 None，不写 stdout，exit 0。
    match bridge_client::send(&enriched_json, blocking).await {
        Some(response) => {
            print!("{response}");
            0
        }
        None => 0,
    }
}

/// 复制原始字段并注入 Bridge 元数据 + 终端环境 + 标准化字段名
pub fn enrich_payload(
    root: &Map<String, Value>,
    source: &str,
    parent_pid: u32,
    tracked: &TrackedProcess,
    terminal_env: &[(String, String)],
) -> Map<String, Value> {
    let mut payload = root.clone();

    payload.insert("_source".to_string(), Value::String(source.to_string()));
    payload.insert("_ppid".to_string(), Value::from(parent_pid));
    payload.insert("_hook_ppid".to_string(), Value::from(std::process::id()));
    payload.insert("_tracked_pid".to_string(), Value::from(tracked.pid));
    payload.insert(
        "_tracked_pid_kind".to_string(),
        Value::String(tracked.kind.to_string()),
    );
    if let Some(started_at) = tracked.started_at_utc {
        payload.insert(
            "_tracked_process_started_at_utc".to_string(),
            Value::String(started_at.to_rfc3339()),
        );
    }

    environment_collector::inject_into_payload(&mut payload, terminal_env);
    field_normalizer::normalize_field_names(&mut payload);

    payload
}

/// 从命令行参数提取 `--source <value>`
fn parse_args_for_source(args: &[String]) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == "--source" && !w[1].is_empty())
        .map(|w| w[1].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn tracked() -> TrackedProcess {
        TrackedProcess {
            pid: 4321,
            kind: "cli",
            started_at_utc: None,
        }
    }

    #[test]
    fn enrich_injects_all_metadata_and_normalizes() {
        let root = map(json!({
            "hookEventName": "PreToolUse",
            "sessionId": "abc"
        }));
        let env = vec![("WT_SESSION".to_string(), "guid".to_string())];

        let payload = enrich_payload(&root, "cursor", 100, &tracked(), &env);

        assert_eq!(payload["_source"], "cursor");
        assert_eq!(payload["_ppid"], 100);
        assert!(payload["_hook_ppid"].is_number());
        assert_eq!(payload["_tracked_pid"], 4321);
        assert_eq!(payload["_tracked_pid_kind"], "cli");
        assert_eq!(payload["_wt_session"], "guid");
        assert_eq!(payload["hook_event_name"], "PreToolUse");
        assert_eq!(payload["session_id"], "abc");
    }

    #[test]
    fn parses_source_arg() {
        let args = vec![
            "bridge".to_string(),
            "--source".to_string(),
            "codex".to_string(),
        ];
        assert_eq!(parse_args_for_source(&args).as_deref(), Some("codex"));
        assert_eq!(parse_args_for_source(&["bridge".to_string()]), None);
    }
}
