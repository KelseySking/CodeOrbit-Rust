//! Hook 安装策略共享辅助 — 路径展开、JSON 读写、bridge 命令解析

use std::fs;
use std::path::Path;
use std::sync::RwLock;

use serde_json::{Value, json};

/// 测试用主目录覆盖环境变量（与 C# 对齐）
const TEST_USERPROFILE_ENV: &str = "CodeOrbit_TEST_USERPROFILE";

/// bridge 可执行文件路径覆盖（由 hub 在运行时设置；默认见 `default_bridge_path`）
static BRIDGE_PATH_OVERRIDE: RwLock<Option<String>> = RwLock::new(None);

fn user_profile_directory() -> String {
    if let Ok(test) = std::env::var(TEST_USERPROFILE_ENV)
        && !test.is_empty()
    {
        return test;
    }
    dirs::home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn app_data_directory() -> String {
    if let Ok(test) = std::env::var(TEST_USERPROFILE_ENV)
        && !test.is_empty()
    {
        return Path::new(&test)
            .join("AppData")
            .join("Roaming")
            .to_string_lossy()
            .into_owned();
    }
    dirs::config_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// 展开路径标记（~/, $HOME, %APPDATA%, %USERPROFILE%）为实际路径
pub fn expand_path(path: &str) -> String {
    let sep = std::path::MAIN_SEPARATOR.to_string();

    if path.starts_with("~/")
        || path.starts_with("~\\")
        || path.starts_with("$HOME/")
        || path.starts_with("$HOME\\")
    {
        let home = user_profile_directory();
        return path
            .replace("~/", &(home.clone() + &sep))
            .replace("~\\", &(home.clone() + &sep))
            .replace("$HOME/", &(home.clone() + &sep))
            .replace("$HOME\\", &(home + &sep));
    }

    if path.starts_with("%APPDATA%/") || path.starts_with("%APPDATA%\\") {
        let app_data = app_data_directory();
        return path
            .replace("%APPDATA%/", &(app_data.clone() + &sep))
            .replace("%APPDATA%\\", &(app_data + &sep));
    }

    if path.starts_with("%USERPROFILE%/") || path.starts_with("%USERPROFILE%\\") {
        let user = user_profile_directory();
        return path
            .replace("%USERPROFILE%/", &(user.clone() + &sep))
            .replace("%USERPROFILE%\\", &(user + &sep));
    }

    path.to_string()
}

/// 确保给定文件路径所在目录存在
pub fn ensure_directory_exists(file_path: &str) {
    if let Some(parent) = Path::new(file_path).parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        let _ = fs::create_dir_all(parent);
    }
}

/// 读取 JSON 文件；文件不存在或解析失败时返回空对象 `{}`
pub fn read_json_file(file_path: &str) -> Value {
    if !Path::new(file_path).exists() {
        return json!({});
    }
    match fs::read_to_string(file_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| json!({})),
        Err(_) => json!({}),
    }
}

/// 将 JSON 值以美化格式写入文件
pub fn write_json_file(file_path: &str, value: &Value) -> std::io::Result<()> {
    ensure_directory_exists(file_path);
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
    fs::write(file_path, text)
}

/// 设置 bridge 可执行文件路径（由 hub 的 ConfigInstaller 在运行时注入）
pub fn set_bridge_executable_path(path: impl Into<String>) {
    if let Ok(mut guard) = BRIDGE_PATH_OVERRIDE.write() {
        *guard = Some(path.into());
    }
}

fn default_bridge_path() -> &'static str {
    if cfg!(windows) {
        "CodeOrbit-bridge.exe"
    } else {
        "CodeOrbit-bridge"
    }
}

/// 获取 bridge 可执行文件路径
pub fn get_bridge_executable_path() -> String {
    BRIDGE_PATH_OVERRIDE
        .read()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| default_bridge_path().to_string())
}

/// 获取给定源 key 的 hook 命令（bridge 路径 + --source，含空格时加引号）
pub fn get_hook_command(source_key: &str) -> String {
    let bridge = get_bridge_executable_path();
    if bridge.contains(' ') {
        format!("\"{bridge}\" --source {source_key}")
    } else {
        format!("{bridge} --source {source_key}")
    }
}

/// 合并两个 JSON 对象，冲突时以 target 为准
pub fn merge_json_objects(source: &Value, target: &Value) -> Value {
    let (Value::Object(src), Value::Object(tgt)) = (source, target) else {
        return target.clone();
    };

    let mut merged = serde_json::Map::new();
    for (k, v) in src {
        if !tgt.contains_key(k) {
            merged.insert(k.clone(), v.clone());
        }
    }
    for (k, v) in tgt {
        merged.insert(k.clone(), v.clone());
    }
    Value::Object(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_command_includes_source() {
        let cmd = get_hook_command("claude");
        assert!(cmd.contains("--source claude"));
    }

    #[test]
    fn merge_target_takes_precedence() {
        let source = json!({"a": 1, "b": 2});
        let target = json!({"b": 3, "c": 4});
        let merged = merge_json_objects(&source, &target);
        assert_eq!(merged["a"], json!(1));
        assert_eq!(merged["b"], json!(3));
        assert_eq!(merged["c"], json!(4));
    }
}
