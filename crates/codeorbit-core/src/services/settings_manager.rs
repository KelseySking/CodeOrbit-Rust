//! 设置管理器 — 基于 JSON 文件的键值存储

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// 设置管理器，存储在 `<settings_dir>/settings.json`
pub struct SettingsManager {
    path: PathBuf,
    settings: HashMap<String, Value>,
}

impl SettingsManager {
    pub fn new(settings_dir: Option<PathBuf>) -> Self {
        let dir = settings_dir.unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("CodeOrbit")
        });
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        let settings = Self::load(&path);
        Self { path, settings }
    }

    /// 读取设置，缺失或反序列化失败时返回默认值
    pub fn get<T: DeserializeOwned>(&self, key: &str, default: T) -> T {
        match self.settings.get(key) {
            Some(value) => serde_json::from_value(value.clone()).unwrap_or(default),
            None => default,
        }
    }

    /// 写入设置并持久化
    pub fn set<T: Serialize>(&mut self, key: &str, value: T) {
        let value = serde_json::to_value(value).unwrap_or(Value::Null);
        self.settings.insert(key.to_string(), value);
        self.save();
    }

    pub fn has(&self, key: &str) -> bool {
        self.settings.contains_key(key)
    }

    pub fn remove(&mut self, key: &str) {
        self.settings.remove(key);
        self.save();
    }

    fn load(path: &PathBuf) -> HashMap<String, Value> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.settings) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_round_trip() {
        let dir = std::env::temp_dir().join(format!("codeorbit-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut mgr = SettingsManager::new(Some(dir.clone()));
        assert_eq!(mgr.get("volume", 50_i64), 50);
        mgr.set("volume", 80_i64);
        assert!(mgr.has("volume"));
        assert_eq!(mgr.get("volume", 50_i64), 80);

        // 重新加载验证持久化
        let mgr2 = SettingsManager::new(Some(dir.clone()));
        assert_eq!(mgr2.get("volume", 0_i64), 80);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
