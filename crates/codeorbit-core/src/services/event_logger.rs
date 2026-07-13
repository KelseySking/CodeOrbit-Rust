//! 轻量级诊断日志：管道分隔字段、线程安全、按大小轮转
//!
//! - `hook.log`：hook 事件诊断（既有）
//! - `error.log`：错误溯源（API / 业务失败 / hook·ipc）

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const DEFAULT_MAX_BYTES: u64 = 1_048_576;

/// 日志类型 → 独立文件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Hook,
    Error,
}

impl LogKind {
    fn file_name(self) -> &'static str {
        match self {
            LogKind::Hook => "hook.log",
            LogKind::Error => "error.log",
        }
    }

    fn rotated_name(self) -> &'static str {
        match self {
            LogKind::Hook => "hook.log.1",
            LogKind::Error => "error.log.1",
        }
    }
}

/// 诊断日志写入器，落地在 `<config>/CodeOrbit/`
pub struct EventLogger {
    dir: PathBuf,
    max_bytes: u64,
    write_lock: Mutex<()>,
}

impl EventLogger {
    /// 自定义目录与轮转阈值
    pub fn new(log_dir: Option<PathBuf>, max_bytes: u64) -> Self {
        let dir = log_dir.unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("CodeOrbit")
        });
        let _ = fs::create_dir_all(&dir);
        Self {
            dir,
            max_bytes,
            write_lock: Mutex::new(()),
        }
    }

    /// 使用默认目录与 1MB 轮转阈值
    pub fn with_defaults() -> Self {
        Self::new(None, DEFAULT_MAX_BYTES)
    }

    pub fn log_dir(&self) -> &Path {
        &self.dir
    }

    pub fn log_path(&self, kind: LogKind) -> PathBuf {
        self.dir.join(kind.file_name())
    }

    /// 兼容旧 API：hook.log 路径
    pub fn hook_log_path(&self) -> PathBuf {
        self.log_path(LogKind::Hook)
    }

    /// 写一行：timestamp|category|message|key1=val1|...
    /// 不抛错 —— 诊断日志失败不能影响主流程。
    pub fn write(&self, kind: LogKind, category: &str, message: &str, fields: &[(&str, &str)]) {
        let mut line = String::with_capacity(256);
        line.push_str(
            &chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
        );
        line.push('|');
        line.push_str(category);
        line.push('|');
        line.push_str(&escape(message));
        for (key, value) in fields {
            line.push('|');
            line.push_str(key);
            line.push('=');
            line.push_str(&escape(value));
        }
        line.push('\n');

        let path = self.log_path(kind);
        let rotated = self.dir.join(kind.rotated_name());
        let _guard = self.write_lock.lock();
        self.rotate_if_needed(&path, &rotated);
        let _ = append(&path, &line);
    }

    /// 兼容旧 API：写入 hook.log
    pub fn write_hook(&self, category: &str, message: &str, fields: &[(&str, &str)]) {
        self.write(LogKind::Hook, category, message, fields);
    }

    /// 写入 error.log
    pub fn write_error(&self, category: &str, message: &str, fields: &[(&str, &str)]) {
        self.write(LogKind::Error, category, message, fields);
    }

    fn rotate_if_needed(&self, path: &Path, rotated_path: &Path) {
        let Ok(meta) = fs::metadata(path) else {
            return;
        };
        if meta.len() < self.max_bytes {
            return;
        }
        if rotated_path.exists() {
            let _ = fs::remove_file(rotated_path);
        }
        let _ = fs::rename(path, rotated_path);
    }
}

/// 进程内默认实例（同目录，避免各处 new 路径不一致）
pub fn global() -> &'static EventLogger {
    static GLOBAL: OnceLock<EventLogger> = OnceLock::new();
    GLOBAL.get_or_init(EventLogger::with_defaults)
}

/// 快捷写 error.log
pub fn log_error(category: &str, message: &str, fields: &[(&str, &str)]) {
    global().write_error(category, message, fields);
}

fn append(path: &Path, content: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(content.as_bytes())
}

fn escape(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if !value.contains(['|', '\n', '\r']) {
        return value.to_string();
    }
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_rotates_hook() {
        let dir = std::env::temp_dir().join(format!("codeorbit-log-hook-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let logger = EventLogger::new(Some(dir.clone()), 64);
        for i in 0..50 {
            logger.write_hook("test", &format!("message {i}"), &[("k", "v")]);
        }
        assert!(dir.join("hook.log").exists());
        assert!(dir.join("hook.log.1").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_error_file_separately() {
        let dir = std::env::temp_dir().join(format!("codeorbit-log-err-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let logger = EventLogger::new(Some(dir.clone()), DEFAULT_MAX_BYTES);
        logger.write_error(
            "api",
            "Pending action not found",
            &[("code", "not_found"), ("status", "404")],
        );
        let path = dir.join("error.log");
        assert!(path.exists());
        assert!(!dir.join("hook.log").exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("api"));
        assert!(content.contains("not_found"));
        assert!(content.contains("Pending action not found"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotates_error_independently() {
        let dir = std::env::temp_dir().join(format!("codeorbit-log-rot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let logger = EventLogger::new(Some(dir.clone()), 64);
        for i in 0..50 {
            logger.write_error("api", &format!("err {i}"), &[]);
        }
        assert!(dir.join("error.log").exists());
        assert!(dir.join("error.log.1").exists());
        assert!(!dir.join("hook.log").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn escapes_pipe_and_newline() {
        assert_eq!(escape("a|b"), "a\\|b");
        assert_eq!(escape("a\nb"), "a\\nb");
        assert_eq!(escape("plain"), "plain");
    }
}
