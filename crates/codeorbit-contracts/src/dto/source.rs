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
    /// WSL 状态查询时填充；Windows 侧为 `null`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// WSL 探测是否成功。`false` 时 `installed` 不可信
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_ok: Option<bool>,
    /// 探测/查询失败原因（不伪装成「未安装」）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 源操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceOperationResultDto {
    pub source: String,
    pub success: bool,
    pub installed: bool,
    pub message: String,
    /// WSL 操作时填充实际使用的发行版名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// 稳定错误码，成功时为 `null`
    ///
    /// 常见值：`unsupported_source` / `invalid_distro` / `missing_bridge` /
    /// `wsl_unavailable` / `hook_write_failed` / `operation_failed`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// 单个 WSL 发行版（用户侧，已过滤 Docker 系统 distro）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WslDistroDto {
    pub name: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    pub is_default: bool,
}

/// `GET /api/sources/wsl/distros` 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WslDistrosDto {
    pub distros: Vec<WslDistroDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_distro: Option<String>,
}
