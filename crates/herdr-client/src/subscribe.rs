//! `events.subscribe` long-lived stream.
//!
//! Wire:
//! - request: `{id, method:"events.subscribe", params:{subscriptions:[{type:"workspace.created"},…]}}`
//! - ack: `{id, result:{type:"subscription_started"}}`
//! - push: `{event:"workspace_created", data:{…}}`  (underscore event names)

use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::resolve::SocketTarget;
use crate::stream::NdjsonStream;
use crate::{ClientError, Result};

/// Default lifecycle subscriptions (no pane_id required).
///
/// Scoped events (`pane.agent_status_changed`, `pane.scroll_changed`,
/// `pane.output_matched`) need per-pane params — add via
/// [`default_subscriptions_with_panes`].
pub fn default_lifecycle_subscriptions() -> Vec<Value> {
    const TYPES: &[&str] = &[
        "workspace.created",
        "workspace.updated",
        "workspace.renamed",
        "workspace.moved",
        "workspace.closed",
        "workspace.focused",
        "worktree.created",
        "worktree.opened",
        "worktree.removed",
        "tab.created",
        "tab.closed",
        "tab.focused",
        "tab.renamed",
        "tab.moved",
        "pane.created",
        "pane.closed",
        "pane.focused",
        "pane.moved",
        "pane.exited",
        "pane.agent_detected",
        "layout.updated",
    ];
    TYPES.iter().map(|t| json!({ "type": t })).collect()
}

/// Lifecycle + agent status watches for known panes.
pub fn default_subscriptions_with_panes(pane_ids: &[String]) -> Vec<Value> {
    let mut subs = default_lifecycle_subscriptions();
    for pane_id in pane_ids {
        subs.push(json!({
            "type": "pane.agent_status_changed",
            "pane_id": pane_id,
        }));
    }
    subs
}

/// Pushed event from a subscription stream.
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionPush {
    /// Underscore form, e.g. `workspace_created`.
    pub event: String,
    #[serde(default)]
    pub data: Value,
}

/// Active subscription connection.
pub struct EventSubscription {
    stream: NdjsonStream,
}

impl EventSubscription {
    /// Connect, send subscribe, wait for `subscription_started`.
    pub async fn start(target: SocketTarget, subscriptions: Vec<Value>) -> Result<Self> {
        let mut stream = NdjsonStream::connect(target).await?;
        let id = uuid::Uuid::new_v4().to_string();
        let req = json!({
            "id": id,
            "method": "events.subscribe",
            "params": { "subscriptions": subscriptions },
        });
        let raw = serde_json::to_vec(&req)?;
        stream.write_line(&raw).await?;

        let ack_bytes = stream.read_line().await?;
        let ack: Value = serde_json::from_slice(&ack_bytes)?;
        if let Some(err) = ack.get("error") {
            return Err(ClientError::Rpc {
                code: err
                    .get("code")
                    .and_then(|c| c.as_str())
                    .unwrap_or("error")
                    .to_string(),
                message: err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("subscribe failed")
                    .to_string(),
            });
        }
        let result_type = ack
            .pointer("/result/type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if result_type != "subscription_started" {
            return Err(ClientError::Message(format!(
                "unexpected subscribe ack: {}",
                String::from_utf8_lossy(&ack_bytes)
            )));
        }
        info!(%id, "events.subscribe started");
        Ok(Self { stream })
    }

    /// Block until the next pushed event line.
    pub async fn next_event(&mut self) -> Result<SubscriptionPush> {
        loop {
            let line = self.stream.read_line().await?;
            debug!(
                line = %String::from_utf8_lossy(&line).trim(),
                "subscribe line"
            );
            // Skip empty lines
            if line.iter().all(|b| b.is_ascii_whitespace()) {
                continue;
            }
            // Could be a stray response — only accept push shape.
            if let Ok(push) = serde_json::from_slice::<SubscriptionPush>(&line) {
                if !push.event.is_empty() {
                    return Ok(push);
                }
            }
            // Ignore non-push JSON (shouldn't happen after ack).
            debug!("ignoring non-push subscribe line");
        }
    }

    pub async fn close(self) -> Result<()> {
        self.stream.close().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_has_workspace_created() {
        let s = default_lifecycle_subscriptions();
        assert!(s.iter().any(|v| v["type"] == "workspace.created"));
        assert!(!s.iter().any(|v| v["type"] == "pane.agent_status_changed"));
    }

    #[test]
    fn panes_add_status_subs() {
        let s = default_subscriptions_with_panes(&["w1:p1".into()]);
        assert!(s
            .iter()
            .any(|v| { v["type"] == "pane.agent_status_changed" && v["pane_id"] == "w1:p1" }));
    }

    #[test]
    fn parse_push() {
        let raw = r#"{"event":"workspace_created","data":{"type":"workspace_created","workspace":{"workspace_id":"w1"}}}"#;
        let p: SubscriptionPush = serde_json::from_str(raw).unwrap();
        assert_eq!(p.event, "workspace_created");
    }
}
