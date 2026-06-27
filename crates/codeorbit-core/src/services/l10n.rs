//! 国际化服务 — 支持 en/zh/ja/ko/tr 五种语言

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// 本地化服务实例
pub struct L10n {
    language: String,
}

impl L10n {
    pub fn new() -> Self {
        Self {
            language: "zh".to_string(),
        }
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn set_language(&mut self, language: impl Into<String>) {
        self.language = language.into();
    }

    /// 解析后的实际语言（"system" → 依据环境探测，否则即设定值）
    pub fn effective_language(&self) -> String {
        if self.language == "system" {
            detect_system_language()
        } else {
            self.language.clone()
        }
    }

    /// 查表翻译；缺失时返回 key 本身
    pub fn get(&self, key: &str) -> String {
        let lang = self.effective_language();
        let table = translations();
        table
            .get(lang.as_str())
            .and_then(|dict| dict.get(key))
            .or_else(|| table["en"].get(key))
            .map(|s| s.to_string())
            .unwrap_or_else(|| key.to_string())
    }
}

impl Default for L10n {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局单例
pub fn instance() -> &'static RwLock<L10n> {
    static INSTANCE: OnceLock<RwLock<L10n>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(L10n::new()))
}

fn detect_system_language() -> String {
    let raw = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();
    let code: String = raw.chars().take(2).collect::<String>().to_lowercase();
    if code.is_empty() {
        "en".to_string()
    } else {
        code
    }
}

type Table = HashMap<&'static str, HashMap<&'static str, &'static str>>;

fn translations() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(build_translations)
}

fn build_translations() -> Table {
    let mut table = HashMap::new();

    table.insert(
        "en",
        dict(&[
            ("app.name", "CodeOrbit"),
            ("panel.noSessions", "No active sessions"),
            ("panel.sessionCount", "{0} active sessions"),
            ("panel.oneSession", "1 active session"),
            ("approval.title", "Permission Request"),
            ("approval.deny", "DENY"),
            ("approval.dismiss", "DISMISS"),
            ("approval.allowOnce", "ALLOW ONCE"),
            ("approval.alwaysAllow", "ALWAYS ALLOW"),
            ("question.skip", "SKIP"),
            ("question.submit", "SUBMIT"),
            ("settings.title", "Settings"),
            ("settings.general", "General"),
            ("settings.behavior", "Behavior"),
            ("settings.appearance", "Appearance"),
            ("settings.mascots", "Mascots"),
            ("settings.sound", "Sound"),
            ("settings.hooks", "Tool Connections"),
            ("settings.about", "About"),
            ("settings.language", "Language"),
            ("settings.launchAtLogin", "Launch at login"),
            ("settings.autoApprove", "Auto-approve safe tools"),
            ("settings.soundEnabled", "Enable sound effects"),
            ("settings.volume", "Volume"),
            ("tray.tooltip", "CodeOrbit"),
            ("tray.show", "Show Panel"),
            ("tray.settings", "Settings"),
            ("tray.quit", "Quit"),
            ("status.idle", "Idle"),
            ("status.processing", "Processing"),
            ("status.running", "Running"),
            ("status.waitingApproval", "Waiting for approval"),
            ("status.waitingQuestion", "Waiting for answer"),
        ]),
    );

    table.insert(
        "zh",
        dict(&[
            ("app.name", "CodeOrbit"),
            ("panel.noSessions", "没有活跃会话"),
            ("panel.sessionCount", "{0} 个活跃会话"),
            ("panel.oneSession", "1 个活跃会话"),
            ("approval.title", "权限请求"),
            ("approval.deny", "拒绝"),
            ("approval.dismiss", "忽略"),
            ("approval.allowOnce", "允许一次"),
            ("approval.alwaysAllow", "始终允许"),
            ("question.skip", "跳过"),
            ("question.submit", "提交"),
            ("settings.title", "设置"),
            ("settings.general", "通用"),
            ("settings.behavior", "行为"),
            ("settings.appearance", "外观"),
            ("settings.mascots", "吉祥物"),
            ("settings.sound", "音效"),
            ("settings.hooks", "工具连接"),
            ("settings.about", "关于"),
            ("settings.language", "语言"),
            ("settings.launchAtLogin", "开机自启"),
            ("settings.autoApprove", "自动审批安全工具"),
            ("settings.soundEnabled", "启用音效"),
            ("settings.volume", "音量"),
            ("tray.tooltip", "CodeOrbit"),
            ("tray.show", "显示面板"),
            ("tray.settings", "设置"),
            ("tray.quit", "退出"),
            ("status.idle", "空闲"),
            ("status.processing", "处理中"),
            ("status.running", "运行中"),
            ("status.waitingApproval", "等待审批"),
            ("status.waitingQuestion", "等待回答"),
        ]),
    );

    table.insert(
        "ja",
        dict(&[
            ("app.name", "CodeOrbit"),
            ("panel.noSessions", "アクティブなセッションなし"),
            ("approval.title", "権限リクエスト"),
            ("approval.deny", "拒否"),
            ("approval.allowOnce", "1回許可"),
            ("approval.alwaysAllow", "常に許可"),
            ("tray.show", "パネルを表示"),
            ("tray.settings", "設定"),
            ("tray.quit", "終了"),
        ]),
    );

    table.insert(
        "ko",
        dict(&[
            ("app.name", "CodeOrbit"),
            ("panel.noSessions", "활성 세션 없음"),
            ("approval.title", "권한 요청"),
            ("approval.deny", "거부"),
            ("approval.allowOnce", "한번 허용"),
            ("approval.alwaysAllow", "항상 허용"),
            ("tray.show", "패널 표시"),
            ("tray.settings", "설정"),
            ("tray.quit", "종료"),
        ]),
    );

    table.insert(
        "tr",
        dict(&[
            ("app.name", "CodeOrbit"),
            ("panel.noSessions", "Aktif oturum yok"),
            ("approval.title", "İzin İsteği"),
            ("approval.deny", "REDDET"),
            ("approval.allowOnce", "BİR KEZ İZİN VER"),
            ("approval.alwaysAllow", "HER ZAMAN İZİN VER"),
            ("tray.show", "Paneli Göster"),
            ("tray.settings", "Ayarlar"),
            ("tray.quit", "Çıkış"),
        ]),
    );

    table
}

fn dict(pairs: &[(&'static str, &'static str)]) -> HashMap<&'static str, &'static str> {
    pairs.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_translations_with_fallback() {
        let mut l10n = L10n::new();
        l10n.set_language("zh");
        assert_eq!(l10n.get("approval.deny"), "拒绝");

        l10n.set_language("en");
        assert_eq!(l10n.get("approval.deny"), "DENY");

        // ja 缺失的 key 回退到 en
        l10n.set_language("ja");
        assert_eq!(l10n.get("question.skip"), "SKIP");

        // 完全未知的 key 返回自身
        assert_eq!(l10n.get("nonexistent.key"), "nonexistent.key");
    }
}
