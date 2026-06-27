use crate::models::{PermissionRequest, QuestionData};

/// 副作用类型 — reducer 产生的 UI 动作
#[derive(Debug, Clone)]
pub enum SideEffect {
    None,
    PlaySound {
        sound_name: String,
    },
    ShowApprovalCard {
        session_id: String,
        request: PermissionRequest,
    },
    ShowQuestionCard {
        session_id: String,
        question: QuestionData,
    },
    JumpToTerminal {
        session_id: String,
    },
    SendResponse {
        response_json: String,
    },
}

impl SideEffect {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}
