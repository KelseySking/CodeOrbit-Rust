use serde::{Deserialize, Serialize};

/// 问题选项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOptionDto {
    pub label: String,
    pub description: Option<String>,
    pub value: Option<String>,
}

/// 问题子项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionItemDto {
    pub id: Option<String>,
    pub question: String,
    pub header: Option<String>,
    pub options: Vec<QuestionOptionDto>,
    pub multi_select: bool,
    pub allow_free_text: bool,
}

/// 问题数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionDto {
    pub session_id: String,
    pub id: Option<String>,
    pub question: String,
    pub header: Option<String>,
    pub options: Vec<QuestionOptionDto>,
    pub multi_select: bool,
    pub is_multi_question: bool,
    pub questions: Vec<QuestionItemDto>,
    pub hook_event_name: String,
    pub is_ask_user_question: bool,
    pub is_codex_request_user_input: bool,
    pub current_question_index: i32,
    pub current_answer_key: String,
}

/// 问题回答请求 (客户端 → 服务端)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionAnswerRequest {
    pub answer: Option<String>,
    pub answers: Option<std::collections::HashMap<String, Vec<String>>>,
    pub actor: Option<String>,
}

/// 当前问题回答请求
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionCurrentAnswerRequest {
    pub answers: Vec<String>,
    pub actor: Option<String>,
}

/// 当前问题回答结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionCurrentAnswerResultDto {
    pub success: bool,
    pub resolved: bool,
}
