//! Unary Herdr operations used by the Phase 1 control plane.

use serde_json::{json, Value};

use crate::{HerdrClient, Result, Transport};

#[derive(Debug, Clone, Copy, Default)]
pub struct WorktreeCreateRequest<'a> {
    pub branch: Option<&'a str>,
    pub path: Option<&'a str>,
    pub base: Option<&'a str>,
    pub label: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub workspace_id: Option<&'a str>,
    pub focus: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorktreeOpenRequest<'a> {
    pub branch: Option<&'a str>,
    pub path: Option<&'a str>,
    pub label: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub workspace_id: Option<&'a str>,
    pub focus: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LayoutApplyRequest<'a> {
    pub root: Value,
    pub tab_label: Option<&'a str>,
    pub workspace_id: Option<&'a str>,
    pub focus: bool,
}

pub fn worktree_create_params(req: WorktreeCreateRequest<'_>) -> Value {
    let mut params = json!({ "focus": req.focus });
    insert_optional_str(&mut params, "branch", req.branch);
    insert_optional_str(&mut params, "path", req.path);
    insert_optional_str(&mut params, "base", req.base);
    insert_optional_str(&mut params, "label", req.label);
    insert_optional_str(&mut params, "cwd", req.cwd);
    insert_optional_str(&mut params, "workspace_id", req.workspace_id);
    params
}

pub fn worktree_open_params(req: WorktreeOpenRequest<'_>) -> Value {
    let mut params = json!({ "focus": req.focus });
    insert_optional_str(&mut params, "branch", req.branch);
    insert_optional_str(&mut params, "path", req.path);
    insert_optional_str(&mut params, "label", req.label);
    insert_optional_str(&mut params, "cwd", req.cwd);
    insert_optional_str(&mut params, "workspace_id", req.workspace_id);
    params
}

pub fn worktree_remove_params(workspace_id: &str, force: bool) -> Value {
    json!({ "workspace_id": workspace_id, "force": force })
}

pub fn layout_apply_params(req: LayoutApplyRequest<'_>) -> Value {
    let mut params = json!({
        "root": req.root,
        "tab_id": null,
        "focus": req.focus,
    });
    insert_optional_str(&mut params, "tab_label", req.tab_label);
    insert_optional_str(&mut params, "workspace_id", req.workspace_id);
    params
}

fn insert_optional_str(params: &mut Value, key: &str, value: Option<&str>) {
    if let Some(v) = value {
        params[key] = json!(v);
    }
}

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

    pub async fn worktree_create(&mut self, req: WorktreeCreateRequest<'_>) -> Result<Value> {
        self.request("worktree.create", Some(worktree_create_params(req)))
            .await
    }

    pub async fn worktree_open(&mut self, req: WorktreeOpenRequest<'_>) -> Result<Value> {
        self.request("worktree.open", Some(worktree_open_params(req)))
            .await
    }

    pub async fn worktree_remove(&mut self, workspace_id: &str, force: bool) -> Result<Value> {
        self.request(
            "worktree.remove",
            Some(worktree_remove_params(workspace_id, force)),
        )
        .await
    }

    pub async fn layout_apply(&mut self, req: LayoutApplyRequest<'_>) -> Result<Value> {
        self.request("layout.apply", Some(layout_apply_params(req)))
            .await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_create_params_include_only_explicit_fields() {
        let params = worktree_create_params(WorktreeCreateRequest {
            branch: Some("feature"),
            path: Some("../feature"),
            base: Some("main"),
            focus: true,
            ..Default::default()
        });

        assert_eq!(params["branch"], "feature");
        assert_eq!(params["path"], "../feature");
        assert_eq!(params["base"], "main");
        assert_eq!(params["focus"], true);
        assert!(params.get("label").is_none());
    }

    #[test]
    fn worktree_remove_params_preserve_force_choice() {
        assert_eq!(worktree_remove_params("ws-1", false)["force"], false);
        assert_eq!(worktree_remove_params("ws-1", true)["force"], true);
    }

    #[test]
    fn layout_apply_params_force_new_tab_contract() {
        let root = json!({
            "type": "split",
            "direction": "right",
            "ratio": 0.5,
            "first": {"type": "pane", "label": "left"},
            "second": {"type": "pane", "label": "right"}
        });

        let params = layout_apply_params(LayoutApplyRequest {
            root,
            tab_label: Some("Dual"),
            workspace_id: Some("ws-1"),
            focus: true,
        });

        assert_eq!(params["tab_id"], Value::Null);
        assert_eq!(params["tab_label"], "Dual");
        assert_eq!(params["workspace_id"], "ws-1");
        assert_eq!(params["focus"], true);
        assert_eq!(params["root"]["type"], "split");
    }
}
