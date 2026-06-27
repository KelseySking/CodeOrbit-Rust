//! 旧版问题响应格式构建器

use serde_json::{Map, Value, json};

use crate::models::QuestionData;

/// 答案集合：有序的 (问题ID, 答案列表)
pub type Answers = [(String, Vec<String>)];

pub(crate) fn build_question_answer_response(question: &QuestionData, answers: &Answers) -> String {
    if answers.len() > 1 || question.is_multi_question {
        let mut answer_object = Map::new();
        for (key, values) in answers {
            answer_object.insert(key.clone(), Value::String(join_answers(values)));
        }
        return json!({ "answers": Value::Object(answer_object) }).to_string();
    }

    let answer = answers
        .first()
        .map(|(_, values)| join_answers(values))
        .unwrap_or_default();
    json!({ "answer": answer }).to_string()
}

pub(crate) fn build_question_dismiss_response(reason: &str) -> String {
    json!({
        "decision": "dismiss",
        "allow": false,
        "reason": reason,
    })
    .to_string()
}

pub(crate) fn join_answers(answers: &[String]) -> String {
    answers.join(", ")
}
