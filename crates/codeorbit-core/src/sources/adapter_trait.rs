//! 源适配器 — trait 定义、插件适配器、内置回退适配器

use std::collections::HashMap;

use super::plugin_models::{DetectionRule, HookInstallationSpec, PermissionResponseStyle};

/// 源适配器：承载来源元数据、事件名标准化与 permission 响应风格。
/// 可选地携带检测规则与 hook 安装规格（插件来源）。
pub trait SourceAdapter: Send + Sync {
    fn source_key(&self) -> &str;
    fn display_name(&self) -> &str;
    fn icon_name(&self) -> &str;
    fn permission_response_style(&self) -> PermissionResponseStyle;

    /// 尝试将原始事件名标准化为标准事件名；无映射返回 None
    fn try_normalize_event_name(&self, raw_event_name: &str) -> Option<String>;

    /// 检测规则（无则 None）
    fn detection_rule(&self) -> Option<&DetectionRule> {
        None
    }

    /// hook 安装规格（无则 None）
    fn hook_installation_spec(&self) -> Option<&HookInstallationSpec> {
        None
    }
}

/// 从 JSON 文件加载的插件定义源适配器
pub struct PluginSourceAdapter {
    source_key: String,
    display_name: String,
    icon_name: String,
    permission_response_style: PermissionResponseStyle,
    event_aliases: HashMap<String, String>,
    detection_rule: Option<DetectionRule>,
    hook_installation_spec: Option<HookInstallationSpec>,
    file_path: Option<String>,
}

impl PluginSourceAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_key: impl Into<String>,
        display_name: impl Into<String>,
        icon_name: impl Into<String>,
        permission_response_style: PermissionResponseStyle,
        event_aliases: HashMap<String, String>,
        detection_rule: Option<DetectionRule>,
        hook_installation_spec: Option<HookInstallationSpec>,
        file_path: Option<String>,
    ) -> Result<Self, String> {
        let source_key = source_key.into();
        let display_name = display_name.into();
        let icon_name = icon_name.into();

        if source_key.trim().is_empty() {
            return Err("Source key cannot be null or whitespace.".to_string());
        }
        if display_name.trim().is_empty() {
            return Err("Display name cannot be null or whitespace.".to_string());
        }
        if icon_name.trim().is_empty() {
            return Err("Icon name cannot be null or whitespace.".to_string());
        }

        Ok(Self {
            source_key,
            display_name,
            icon_name,
            permission_response_style,
            event_aliases,
            detection_rule,
            hook_installation_spec,
            file_path,
        })
    }

    /// 插件来源 JSON 文件路径
    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }
}

impl SourceAdapter for PluginSourceAdapter {
    fn source_key(&self) -> &str {
        &self.source_key
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn icon_name(&self) -> &str {
        &self.icon_name
    }

    fn permission_response_style(&self) -> PermissionResponseStyle {
        self.permission_response_style
    }

    fn try_normalize_event_name(&self, raw_event_name: &str) -> Option<String> {
        let key = raw_event_name.trim();
        self.event_aliases
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone())
    }

    fn detection_rule(&self) -> Option<&DetectionRule> {
        self.detection_rule.as_ref()
    }

    fn hook_installation_spec(&self) -> Option<&HookInstallationSpec> {
        self.hook_installation_spec.as_ref()
    }
}

/// 内置源适配器（用于未知来源的回退）
pub struct BuiltInSourceAdapter {
    source_key: String,
    display_name: String,
    icon_name: String,
    permission_response_style: PermissionResponseStyle,
}

impl BuiltInSourceAdapter {
    pub fn new(
        source_key: impl Into<String>,
        display_name: impl Into<String>,
        icon_name: impl Into<String>,
        permission_response_style: PermissionResponseStyle,
    ) -> Self {
        Self {
            source_key: source_key.into(),
            display_name: display_name.into(),
            icon_name: icon_name.into(),
            permission_response_style,
        }
    }
}

impl SourceAdapter for BuiltInSourceAdapter {
    fn source_key(&self) -> &str {
        &self.source_key
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn icon_name(&self) -> &str {
        &self.icon_name
    }

    fn permission_response_style(&self) -> PermissionResponseStyle {
        self.permission_response_style
    }

    fn try_normalize_event_name(&self, _raw_event_name: &str) -> Option<String> {
        None
    }
}
