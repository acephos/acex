//! Unary Herdr operations used by the Phase 1 control plane.

use serde_json::{json, Value};

use crate::{HerdrClient, Result, Transport};

impl<T: Transport> HerdrClient<T> {
    pub async fn agent_list(&mut self) -> Result<Value> {
        self.request("agent.list", Some(json!({}))).await
    }

    pub async fn agent_get(&mut self, target: &str) -> Result<Value> {
        self.request("agent.get", Some(json!({ "target": target })))
            .await
    }

    pub async fn agent_focus(&mut self, target: &str) -> Result<Value> {
        self.request("agent.focus", Some(json!({ "target": target })))
            .await
    }

    pub async fn agent_send(&mut self, target: &str, text: &str) -> Result<Value> {
        self.request(
            "agent.send",
            Some(json!({ "target": target, "text": text })),
        )
        .await
    }

    pub async fn agent_read(
        &mut self,
        target: &str,
        source: &str,
        lines: u32,
        strip_ansi: bool,
    ) -> Result<Value> {
        self.request(
            "agent.read",
            Some(json!({
                "target": target,
                "source": source,
                "lines": lines,
                "strip_ansi": strip_ansi,
                "format": "text",
            })),
        )
        .await
    }

    pub async fn pane_read(
        &mut self,
        pane_id: &str,
        source: &str,
        lines: u32,
        strip_ansi: bool,
    ) -> Result<Value> {
        self.request(
            "pane.read",
            Some(json!({
                "pane_id": pane_id,
                "source": source,
                "lines": lines,
                "strip_ansi": strip_ansi,
                "format": "text",
            })),
        )
        .await
    }

    pub async fn agent_start(
        &mut self,
        name: &str,
        argv: &[String],
        cwd: Option<&str>,
        focus: bool,
    ) -> Result<Value> {
        let mut params = json!({
            "name": name,
            "argv": argv,
            "focus": focus,
        });
        if let Some(c) = cwd {
            params["cwd"] = json!(c);
        }
        self.request("agent.start", Some(params)).await
    }

    pub async fn worktree_list(
        &mut self,
        workspace_id: Option<&str>,
        cwd: Option<&str>,
    ) -> Result<Value> {
        let mut params = json!({});
        if let Some(w) = workspace_id {
            params["workspace_id"] = json!(w);
        }
        if let Some(c) = cwd {
            params["cwd"] = json!(c);
        }
        self.request("worktree.list", Some(params)).await
    }

    pub async fn notification_show(&mut self, title: &str, body: Option<&str>) -> Result<Value> {
        let mut params = json!({ "title": title });
        if let Some(b) = body {
            params["body"] = json!(b);
        }
        self.request("notification.show", Some(params)).await
    }
}

/// Extract peek text from agent.read / pane.read result shapes.
pub fn extract_read_text(result: &Value) -> String {
    if let Some(t) = result.get("text").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    if let Some(t) = result.pointer("/read/text").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    // Some envelopes wrap as { type, …fields }
    serde_json::to_string_pretty(result).unwrap_or_default()
}

/// Collect pane/agent targets from agent.list result.
pub fn extract_agent_rows(result: &Value) -> Vec<Value> {
    if let Some(arr) = result.get("agents").and_then(|v| v.as_array()) {
        return arr.clone();
    }
    if let Some(arr) = result.as_array() {
        return arr.clone();
    }
    Vec::new()
}
