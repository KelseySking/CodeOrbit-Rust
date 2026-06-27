use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 健康检查响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiHealthDto {
    pub status: String,
    pub started_at_utc: DateTime<Utc>,
}

/// 版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiVersionDto {
    pub product: String,
    pub version: String,
}

/// API 能力声明
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCapabilitiesDto {
    pub hook_injection: bool,
    pub approval: bool,
    pub question: bool,
    pub transcript: bool,
    pub realtime: bool,
    pub realtime_protocols: Vec<String>,
    pub security_mode: String,
}

/// API 错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorDto {
    pub code: String,
    pub message: String,
}

/// 运行时资源信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAssetsDto {
    pub runtime_directory: String,
    pub hook_script_path: String,
    pub bridge_exe_path: String,
    pub installed: bool,
}
