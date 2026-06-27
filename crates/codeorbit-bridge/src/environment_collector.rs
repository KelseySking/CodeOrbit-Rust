//! 终端环境变量收集与注入

use serde_json::{Map, Value};

/// 采集的终端环境变量键（与 C# 实现一致）
const ENV_KEYS: &[&str] = &[
    "WT_SESSION",
    "TERM_PROGRAM",
    "TERMINAL_EMULATOR",
    "TERM_SESSION_ID",
    "KITTY_WINDOW_ID",
    "TMUX",
    "TMUX_PANE",
    "WEZTERM_PANE",
    "VSCODE_INJECTION",
    "VSCODE_GIT_IPC_HANDLE",
    "ConEmuPID",
    "ANSICON",
    "MSYSTEM",
    "SHELL",
    "TERM",
    "COLORTERM",
    "WT_PROFILE_ID",
];

/// 采集所有相关的终端环境变量（保留原始键名与顺序，跳过空值）
pub fn collect() -> Vec<(String, String)> {
    let mut result = Vec::new();
    for key in ENV_KEYS {
        if let Ok(value) = std::env::var(key)
            && !value.is_empty()
        {
            result.push(((*key).to_string(), value));
        }
    }
    // 控制台标题（Windows 特有）
    if let Some(title) = console_title() {
        result.push(("_console_title".to_string(), title));
    }
    // 当前工作目录
    if let Ok(cwd) = std::env::current_dir() {
        result.push(("_cwd".to_string(), cwd.to_string_lossy().into_owned()));
    }
    result
}

/// 将环境变量注入 payload：键名转小写，无 `_` 前缀者补前缀
pub fn inject_into_payload(payload: &mut Map<String, Value>, env: &[(String, String)]) {
    for (key, value) in env {
        let mut normalized = key.to_lowercase();
        if !normalized.starts_with('_') {
            normalized = format!("_{normalized}");
        }
        payload.insert(normalized, Value::String(value.clone()));

        // WT_SESSION 保留原始大写键（终端激活的关键字段）
        if key == "WT_SESSION" && !payload.contains_key("WT_SESSION") {
            payload.insert("WT_SESSION".to_string(), Value::String(value.clone()));
        }
    }
}

#[cfg(windows)]
fn console_title() -> Option<String> {
    use windows_sys::Win32::System::Console::GetConsoleTitleW;
    let mut buf = [0u16; 512];
    // SAFETY: 传入有效缓冲区与其长度
    let len = unsafe { GetConsoleTitleW(buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 {
        return None;
    }
    let title = String::from_utf16_lossy(&buf[..len as usize]);
    if title.is_empty() { None } else { Some(title) }
}

#[cfg(not(windows))]
fn console_title() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn injects_with_underscore_prefix_lowercased() {
        let mut payload = Map::new();
        let env = vec![
            ("WT_SESSION".to_string(), "guid-123".to_string()),
            ("TERM_PROGRAM".to_string(), "vscode".to_string()),
            ("_cwd".to_string(), "/home".to_string()),
        ];
        inject_into_payload(&mut payload, &env);

        assert_eq!(payload["_wt_session"], "guid-123");
        assert_eq!(payload["_term_program"], "vscode");
        // 已有 _ 前缀者不重复加
        assert_eq!(payload["_cwd"], "/home");
        // WT_SESSION 保留原始大写键
        assert_eq!(payload["WT_SESSION"], "guid-123");
    }

    #[test]
    fn does_not_override_existing_wt_session() {
        let mut payload: Map<String, Value> = json!({ "WT_SESSION": "original" })
            .as_object()
            .unwrap()
            .clone();
        inject_into_payload(
            &mut payload,
            &[("WT_SESSION".to_string(), "new".to_string())],
        );
        assert_eq!(payload["WT_SESSION"], "original");
        assert_eq!(payload["_wt_session"], "new");
    }
}
