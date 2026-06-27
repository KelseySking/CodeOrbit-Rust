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
