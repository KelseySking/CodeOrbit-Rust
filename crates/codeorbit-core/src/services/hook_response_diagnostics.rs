//! Hook 响应诊断 — 判别响应 JSON 的类型

use serde_json::Value;

/// 判别 hook 响应字符串的类型（用于排查 HUD 状态机问题）
pub fn get_response_type(response: Option<&str>) -> String {
    let Some(response) = response else {
        return "empty".to_string();
    };
    if response.trim().is_empty() {
        return "empty".to_string();
    }

    let root: Value = match serde_json::from_str(response) {
        Ok(v) => v,
        Err(_) => return "invalid-json".to_string(),
    };

    let Value::Object(obj) = &root else {
        return "other".to_string();
    };
    if obj.is_empty() {
        return "empty".to_string();
    }

    let Some(Value::Object(output)) = obj.get("hookSpecificOutput") else {
        return "other".to_string();
    };

    if output.contains_key("permissionDecision") {
        return "permissionDecision".to_string();
    }
    if let Some(Value::Object(decision)) = output.get("decision") {
        return if decision.contains_key("updatedInput") {
            "decision.updatedInput".to_string()
        } else {
            "decision".to_string()
        };
    }
    if output.contains_key("updatedInput") {
        return "updatedInput".to_string();
    }
    "hookSpecificOutput".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_response_types() {
        assert_eq!(get_response_type(None), "empty");
        assert_eq!(get_response_type(Some("")), "empty");
        assert_eq!(get_response_type(Some("not json")), "invalid-json");
        assert_eq!(get_response_type(Some("{}")), "empty");
        assert_eq!(
            get_response_type(Some(
                r#"{"hookSpecificOutput":{"permissionDecision":"allow"}}"#
            )),
            "permissionDecision"
        );
        assert_eq!(
            get_response_type(Some(
                r#"{"hookSpecificOutput":{"decision":{"behavior":"allow"}}}"#
            )),
            "decision"
        );
    }
}
