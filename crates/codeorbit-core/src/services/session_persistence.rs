//! 会话持久化 — 保存/加载会话快照到 JSON 文件

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{AgentStatus, SessionSnapshot};

/// 会话持久化器，存储在 `<data_dir>/sessions.json`
pub struct SessionPersistence {
    path: PathBuf,
}

impl SessionPersistence {
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        let dir = data_dir.unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("CodeOrbit")
        });
        let _ = std::fs::create_dir_all(&dir);
        Self {
            path: dir.join("sessions.json"),
        }
    }

    /// 保存会话（仅持久化稳定的元数据字段）
    pub fn save(&self, sessions: &HashMap<String, SessionSnapshot>) -> std::io::Result<()> {
        let data: HashMap<&String, PersistedSession> = sessions
            .iter()
            .map(|(key, snap)| (key, PersistedSession::from_snapshot(snap)))
            .collect();
        let json = serde_json::to_string_pretty(&data)?;
        std::fs::write(&self.path, json)
    }

    /// 加载会话；文件不存在或解析失败时返回空
    pub fn load(&self) -> HashMap<String, SessionSnapshot> {
        let Ok(json) = std::fs::read_to_string(&self.path) else {
            return HashMap::new();
        };
        let Ok(data) = serde_json::from_str::<HashMap<String, PersistedSession>>(&json) else {
            return HashMap::new();
        };
        data.into_iter()
            .map(|(key, persisted)| (key, persisted.into_snapshot()))
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PersistedSession {
    session_id: String,
    source: String,
    project_name: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    last_updated_at: DateTime<Utc>,
}

impl PersistedSession {
    fn from_snapshot(snap: &SessionSnapshot) -> Self {
        Self {
            session_id: snap.session_id.clone(),
            source: snap.source.clone(),
            project_name: snap.project_name.clone(),
            status: snap.status.to_string(),
            created_at: snap.created_at,
            last_updated_at: snap.last_updated_at,
        }
    }

    fn into_snapshot(self) -> SessionSnapshot {
        let mut snap = SessionSnapshot::new(self.session_id, self.source);
        snap.project_name = self.project_name;
        snap.status =
            serde_json::from_value(Value::String(self.status)).unwrap_or(AgentStatus::Idle);
        snap.created_at = self.created_at;
        snap.last_updated_at = self.last_updated_at;
        snap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("codeorbit-sess-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let persistence = SessionPersistence::new(Some(dir.clone()));

        let mut sessions = HashMap::new();
        let mut snap = SessionSnapshot::new("s1".to_string(), "claude".to_string());
        snap.status = AgentStatus::WaitingApproval;
        snap.project_name = Some("proj".to_string());
        sessions.insert("s1".to_string(), snap);

        persistence.save(&sessions).unwrap();
        let loaded = persistence.load();

        assert_eq!(loaded.len(), 1);
        let s = &loaded["s1"];
        assert_eq!(s.source, "claude");
        assert_eq!(s.status, AgentStatus::WaitingApproval);
        assert_eq!(s.project_name.as_deref(), Some("proj"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
