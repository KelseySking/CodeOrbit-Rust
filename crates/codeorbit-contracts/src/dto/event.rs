use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 实时事件广播信封
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubEventDto {
    #[serde(rename = "type")]
    pub event_type: String,
    pub timestamp_utc: DateTime<Utc>,
    pub data: Option<serde_json::Value>,
}
