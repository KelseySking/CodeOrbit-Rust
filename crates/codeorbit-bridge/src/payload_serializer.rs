//! Payload 序列化 — HashMap → 紧凑 JSON（显式类型，无反射）
//!
//! Rust 中 payload 即 `serde_json::Map<String, Value>`，序列化天然显式且紧凑。

use serde_json::{Map, Value};

/// 将富化后的 payload 序列化为紧凑 JSON 字符串
pub fn serialize(payload: &Map<String, Value>) -> String {
    serde_json::to_string(&Value::Object(payload.clone())).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_compact_json() {
        let payload: Map<String, Value> = json!({
            "a": 1,
            "b": "text",
            "c": true,
            "d": null,
            "e": [1, 2],
            "f": { "nested": "x" }
        })
        .as_object()
        .unwrap()
        .clone();

        let out = serialize(&payload);
        // 紧凑：无多余空白
        assert!(!out.contains(": "));
        assert!(!out.contains(", "));
        // 可往返解析
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["b"], "text");
        assert_eq!(parsed["f"]["nested"], "x");
    }
}
