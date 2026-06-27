//! 轻量级 hook 事件诊断日志：管道分隔字段、线程安全、按大小轮转

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const DEFAULT_MAX_BYTES: u64 = 1_048_576;

/// hook 诊断日志写入器，落地在 `<config>/CodeOrbit/hook.log`
pub struct EventLogger {
    log_path: PathBuf,
    rotated_path: PathBuf,
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
            log_path: dir.join("hook.log"),
            rotated_path: dir.join("hook.log.1"),
            max_bytes,
            write_lock: Mutex::new(()),
        }
    }

    /// 使用默认目录与 1MB 轮转阈值
    pub fn with_defaults() -> Self {
        Self::new(None, DEFAULT_MAX_BYTES)
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// 写一行日志：timestamp|category|message|key1=val1|...
    /// 不抛错 —— 诊断日志失败不能影响主流程。
    pub fn write(&self, category: &str, message: &str, fields: &[(&str, &str)]) {
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

        let _guard = self.write_lock.lock();
        self.rotate_if_needed();
        let _ = append(&self.log_path, &line);
    }

    fn rotate_if_needed(&self) {
        let Ok(meta) = fs::metadata(&self.log_path) else {
            return;
        };
        if meta.len() < self.max_bytes {
            return;
        }
        if self.rotated_path.exists() {
            let _ = fs::remove_file(&self.rotated_path);
        }
        let _ = fs::rename(&self.log_path, &self.rotated_path);
    }
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
    fn writes_and_rotates() {
        let dir = std::env::temp_dir().join(format!("codeorbit-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let logger = EventLogger::new(Some(dir.clone()), 64);
        for i in 0..50 {
            logger.write("test", &format!("message {i}"), &[("k", "v")]);
        }
        // 触发了轮转
        assert!(dir.join("hook.log").exists());
        assert!(dir.join("hook.log.1").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn escapes_pipe_and_newline() {
        assert_eq!(escape("a|b"), "a\\|b");
        assert_eq!(escape("a\nb"), "a\\nb");
        assert_eq!(escape("plain"), "plain");
    }
}
