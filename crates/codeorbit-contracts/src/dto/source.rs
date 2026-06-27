use serde::{Deserialize, Serialize};

/// 源能力声明
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapabilitiesDto {
    pub hook_install: bool,
    pub approval: bool,
    pub question: bool,
    pub transcript: bool,
    pub always_allow: bool,
}

/// 源信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDto {
    pub id: String,
    pub display_name: String,
    pub icon_name: String,
    pub installed: bool,
    pub capabilities: SourceCapabilitiesDto,
    pub source_type: String,
}

/// 源状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatusDto {
    pub source: String,
    pub supported: bool,
    pub installed: bool,
    pub display_name: String,
}

/// 源操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceOperationResultDto {
    pub source: String,
    pub success: bool,
    pub installed: bool,
    pub message: String,
}
