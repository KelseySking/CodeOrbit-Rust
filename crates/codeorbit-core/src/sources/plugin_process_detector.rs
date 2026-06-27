//! 进程检测引擎 — 将进程祖先链匹配到插件定义的检测规则

use super::adapter_trait::SourceAdapter;
use super::plugin_models::{DetectionRule, ProcessInfo};
use super::source_plugin_loader::SourcePluginLoader;

/// 基于插件检测规则的 CLI 源检测器
pub struct PluginProcessDetector {
    rules: Vec<DetectionRule>,
}

impl PluginProcessDetector {
    /// 直接以检测规则构造（已按需排序前调用 `detect_*` 会自动排序）
    pub fn new(rules: Vec<DetectionRule>) -> Self {
        Self { rules }
    }

    /// 从加载器的所有插件收集检测规则
    pub fn from_loader(loader: &SourcePluginLoader) -> Self {
        let rules = loader
            .load_plugins()
            .iter()
            .filter_map(|p| p.detection_rule().cloned())
            .collect();
        Self::new(rules)
    }

    /// 从简单进程列表（名称 + 可执行路径）检测来源
    pub fn detect_from_process_list(
        &self,
        processes: &[(String, Option<String>)],
    ) -> Option<String> {
        let ancestry: Vec<ProcessInfo> = processes
            .iter()
            .map(|(name, path)| ProcessInfo::new(name.clone(), path.clone()))
            .collect();
        self.detect_from_ancestry(&ancestry)
    }

    /// 从进程祖先链检测来源；无匹配返回 None。按 priority 降序匹配。
    pub fn detect_from_ancestry(&self, ancestry: &[ProcessInfo]) -> Option<String> {
        let mut rules: Vec<&DetectionRule> = self.rules.iter().collect();
        rules.sort_by_key(|r| std::cmp::Reverse(r.priority));

        for rule in rules {
            if rule.matches(ancestry) {
                return Some(rule.source_key.clone());
            }
        }
        None
    }
}
