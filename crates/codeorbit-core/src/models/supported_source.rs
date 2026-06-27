/// 支持的 AI 工具来源验证
pub struct SupportedSource;

impl SupportedSource {
    /// 所有已知的来源标识
    const KNOWN_SOURCES: &'static [&'static str] = &[
        "claude",
        "codex",
        "cursor",
        "gemini",
        "copilot",
        "qoder",
        "codebuddy",
        "opencode",
        "cline",
        "kiro",
        "trae",
        "droid",
        "hermes",
        "pi",
        "kimi",
        "qwen",
        "stepfun",
        "antigravity",
        "workbuddy",
    ];

    /// 检查来源是否有效
    pub fn is_valid(source: &str) -> bool {
        Self::KNOWN_SOURCES.contains(&source)
    }

    /// 获取显示名称
    pub fn get_display_name(source: &str) -> &'static str {
        match source {
            "claude" => "Claude Code",
            "codex" => "Codex",
            "cursor" => "Cursor",
            "gemini" => "Gemini",
            "copilot" => "GitHub Copilot",
            "qoder" => "Qoder",
            "codebuddy" => "CodeBuddy",
            "opencode" => "OpenCode",
            "cline" => "Cline",
            "kiro" => "Kiro",
            "trae" => "Trae",
            "droid" => "Droid",
            "hermes" => "Hermes",
            "pi" => "Pi",
            "kimi" => "Kimi",
            "qwen" => "Qwen",
            "stepfun" => "StepFun",
            "antigravity" => "AntiGravity",
            "workbuddy" => "WorkBuddy",
            "unknown" => "Unknown Tool",
            "CodeOrbit" => "CodeOrbit",
            _ => "Unknown Tool",
        }
    }

    /// 获取图标名称
    pub fn get_icon_name(source: &str) -> &'static str {
        match source {
            "claude" => "claude",
            "codex" => "codex",
            "cursor" => "cursor",
            "gemini" => "gemini",
            "copilot" => "copilot",
            _ => "default",
        }
    }
}
