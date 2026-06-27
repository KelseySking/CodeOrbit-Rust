//! Runtime Manifest — 可选的运行时配置文件

use std::path::Path;

use serde::Deserialize;

/// runtime-manifest.json 内容
#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)] // schema 字段：当前仅消费 default_port，其余保留以完整反映 manifest
pub struct RuntimeManifest {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub build_date: String,
    #[serde(default)]
    pub default_port: Option<u16>,
    #[serde(default)]
    pub plugin_dirs: Vec<String>,
}

/// 从指定目录加载 `runtime-manifest.json`；缺失或解析失败返回 None
pub fn load(dir: &Path) -> Option<RuntimeManifest> {
    let path = dir.join("runtime-manifest.json");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest() {
        let dir = std::env::temp_dir().join(format!("codeorbit-manifest-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("runtime-manifest.json"),
            r#"{"version":"1.2.3","default_port":40000,"plugin_dirs":["bundled-plugins"]}"#,
        )
        .unwrap();

        let manifest = load(&dir).unwrap();
        assert_eq!(manifest.version, "1.2.3");
        assert_eq!(manifest.default_port, Some(40000));
        assert_eq!(manifest.plugin_dirs, vec!["bundled-plugins"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_manifest_returns_none() {
        let dir = std::env::temp_dir().join("codeorbit-no-manifest-xyz");
        assert!(load(&dir).is_none());
    }
}
