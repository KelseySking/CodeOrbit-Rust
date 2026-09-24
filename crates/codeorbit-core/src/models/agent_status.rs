use serde::{Deserialize, Serialize};

/// AI Agent 生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[derive(Default)]
pub enum AgentStatus {
    #[default]
    Idle,
    Processing,
    Running,
    WaitingQuestion,
    WaitingApproval,
    Completed,
    Error,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Processing => write!(f, "Processing"),
            Self::Running => write!(f, "Running"),
            Self::WaitingQuestion => write!(f, "WaitingQuestion"),
            Self::WaitingApproval => write!(f, "WaitingApproval"),
            Self::Completed => write!(f, "Completed"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// 最近一轮的结果。只有 claude 写，其它源保持 Unspecified。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnOutcome {
    #[default]
    Unspecified,
    Succeeded,
    Failed,
}

/// claude 当前的权限档位。持续状态，不是「正在等人批准」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
}

impl PermissionMode {
    /// 只认 payload 里声明的值。未知字符串返回 None，调用方保持上次的值。
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "default" => Some(Self::Default),
            "acceptEdits" | "accept_edits" => Some(Self::AcceptEdits),
            "bypassPermissions" | "bypass_permissions" => Some(Self::BypassPermissions),
            "plan" => Some(Self::Plan),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let status = AgentStatus::WaitingApproval;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"WaitingApproval\"");
        let parsed: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, AgentStatus::WaitingApproval);
    }
}
