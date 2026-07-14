//! Herdr protocol types — forward-compatible, platform-agnostic.
//!
//! Unknown fields are ignored. Peeks and snapshots are *data*, not UI.
//! Refresh `schemas/` from `herdr api schema --json` when protocol moves.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wire protocol envelope (request).
///
/// Herdr requires `params` to be present (use `{}` when empty).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Wire protocol envelope (response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// Coarse agent lifecycle as shown on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    Idle,
    Working,
    Blocked,
    Done,
    #[default]
    Unknown,
}

impl AgentState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Unknown => "unknown",
        }
    }
}

/// Minimal agent row for the control plane (expand via snapshot fields).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentSummary {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Prefer `state`; Herdr may also send `agent_status`.
    #[serde(default, alias = "agent_status")]
    pub state: AgentState,
    #[serde(default, alias = "pane")]
    pub pane_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// Raw passthrough for fields we have not modeled yet.
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Session snapshot is intentionally loose: full shape evolves with Herdr.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSnapshot {
    #[serde(default)]
    pub workspaces: Vec<Value>,
    #[serde(default)]
    pub tabs: Vec<Value>,
    #[serde(default)]
    pub panes: Vec<Value>,
    #[serde(default)]
    pub agents: Vec<AgentSummary>,
    #[serde(default)]
    pub protocol: Option<u32>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Server-pushed event (subscribe stream).
///
/// Wire shape is `{ "event": "workspace_created", "data": { … } }`
/// (underscore names). Older internal code may still use `type`/`payload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Underscore form from wire (`workspace_created`), or dotted legacy.
    #[serde(default, alias = "type")]
    pub event: String,
    #[serde(default, alias = "payload")]
    pub data: Value,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl Event {
    pub fn kind(&self) -> &str {
        &self.event
    }

    /// Normalize `workspace_created` / `workspace.created` → `workspace_created`.
    pub fn kind_normalized(&self) -> String {
        self.event.replace('.', "_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_state_roundtrip() {
        let s = AgentState::Blocked;
        let v = serde_json::to_value(s).unwrap();
        assert_eq!(v, serde_json::json!("blocked"));
    }

    #[test]
    fn snapshot_ignores_unknown() {
        let raw = r#"{"agents":[{"id":"a1","state":"working","mystery":true}],"future_field":1}"#;
        let snap: SessionSnapshot = serde_json::from_str(raw).unwrap();
        assert_eq!(snap.agents.len(), 1);
        assert_eq!(snap.agents[0].state, AgentState::Working);
    }

    #[test]
    fn event_wire_shape_and_normalize() {
        let raw = r#"{"event":"pane_agent_status_changed","data":{"pane_id":"w1:p1","agent_status":"idle"}}"#;
        let ev: Event = serde_json::from_str(raw).unwrap();
        assert_eq!(ev.kind(), "pane_agent_status_changed");
        assert_eq!(ev.kind_normalized(), "pane_agent_status_changed");
        let dotted = Event {
            event: "workspace.created".into(),
            data: serde_json::json!({}),
            extra: Default::default(),
        };
        assert_eq!(dotted.kind_normalized(), "workspace_created");
    }

    #[test]
    fn agent_status_alias_deserializes() {
        let raw = r#"{"id":"p","agent_status":"blocked","pane_id":"w1:p1"}"#;
        let a: AgentSummary = serde_json::from_str(raw).unwrap();
        assert_eq!(a.state, AgentState::Blocked);
        assert_eq!(a.pane_id.as_deref(), Some("w1:p1"));
    }
}
