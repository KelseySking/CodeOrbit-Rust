//! 插件/源管理系统 — JSON 插件解析、加载、检测、适配器注册与 Hook 安装策略

pub mod adapter_registry;
pub mod adapter_trait;
pub mod hook_installation_utils;
pub mod hook_strategy_factory;
pub mod plugin_models;
pub mod plugin_process_detector;
pub mod plugin_validator;
pub mod source_plugin_json_parser;
pub mod source_plugin_loader;
pub mod strategies;

pub use adapter_registry::SourceAdapterRegistry;
pub use adapter_trait::{BuiltInSourceAdapter, PluginSourceAdapter, SourceAdapter};
pub use plugin_models::{
    DetectionRule, ExtraConfigSpec, HookInstallationSpec, PermissionResponseStyle, PluginMetadata,
    PluginValidationError, ProcessInfo, hook_formats,
};
pub use plugin_process_detector::PluginProcessDetector;
pub use source_plugin_json_parser::{ParseError, parse};
pub use source_plugin_loader::SourcePluginLoader;
pub use strategies::HookInstallationStrategy;
