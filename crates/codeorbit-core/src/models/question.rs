use serde::{Deserialize, Serialize};
use serde_json::Value;

/// AI 工具提出的问题
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuestionData {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    pub options: Option<Vec<QuestionOption>>,
    pub multi_select: bool,
    pub is_multi_question: bool,
    pub questions: Option<Vec<QuestionItem>>,
    pub hook_event_name: String,
    pub is_ask_user_question: bool,
    pub is_codex_request_user_input: bool,
    #[serde(skip)]
    pub original_input: Option<Value>,
}

/// 问题选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// 问题子项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    pub options: Option<Vec<QuestionOption>>,
    pub multi_select: bool,
    pub allow_free_text: bool,
}
