//! In-memory control-plane model.
//!
//! Herdr is authority. This store is a reduce of snapshot + events.

mod intent;

pub use herdr_types::{AgentState, AgentSummary, Event, SessionSnapshot};
pub use intent::{Intent, ZedOpenMode};

use serde_json::Value;

/// Connection health for the status badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnState {
    #[default]
    Offline,
    Connecting,
    Live,
    Reconnecting,
}

/// Filters for the agent board (client-side).
#[derive(Debug, Clone, Default)]
pub struct BoardFilter {
    pub state: Option<AgentState>,
    pub workspace: Option<String>,
    pub query: String,
}

/// Application store reduced from Herdr truth.
#[derive(Debug, Clone, Default)]
pub struct Store {
    pub conn: ConnState,
    pub last_error: Option<String>,
    /// Non-error status line (e.g. snapshot counts).
    pub status_note: Option<String>,
    pub snapshot: SessionSnapshot,
    pub agents: Vec<AgentSummary>,
    pub filter: BoardFilter,
    pub selected: Option<usize>,
    pub live: bool,
    /// True after `events.subscribe` ack (stream open).
    pub subscribed: bool,
    pub workspace_count: usize,
    pub pane_count: usize,
    pub tab_count: usize,
    /// Monotonic count of applied push events.
    pub event_count: u64,
    /// Last event kind (underscore form) for the status strip.
    pub last_event: Option<String>,
    /// Recent event kinds (newest last), for the UI peek.
    pub recent_events: Vec<String>,
    /// How many times the control plane completed a reconnect/resync cycle.
    pub reconnect_count: u64,
    /// Generation of the last applied snapshot (increments on each full replace).
    pub snapshot_gen: u64,
    /// Output peek buffer for the selected (or last peeked) agent.
    pub peek_lines: Vec<String>,
    pub peek_target: Option<String>,
    pub peek_busy: bool,
    /// Worktree list rows (F12) — JSON-ish summaries for display.
    pub worktrees: Vec<String>,
    /// Wait badges: target → desired status (F11).
    pub waits: Vec<WaitBadge>,
    /// Toast-like action feedback (not Herdr notification).
    pub toast: Option<String>,
    pub peek_line_limit: u32,
}

/// Non-blocking wait tracker for a pane/agent.
#[derive(Debug, Clone)]
pub struct WaitBadge {
    pub target: String,
    pub want: AgentState,
    pub armed: bool,
    pub resolved: bool,
    pub message: String,
}

impl Store {
    /// Full replace from `session.snapshot` — authority reset (F04).
    ///
    /// Does **not** replay prior events; subsequent subscribe pushes apply on top.
    pub fn apply_snapshot(&mut self, snap: SessionSnapshot) {
        self.apply_snapshot_inner(snap, false);
    }

    /// Snapshot replace used after reconnect — bumps reconnect metrics.
    pub fn apply_resnapshot(&mut self, snap: SessionSnapshot) {
        self.apply_snapshot_inner(snap, true);
    }

    fn apply_snapshot_inner(&mut self, snap: SessionSnapshot, from_reconnect: bool) {
        self.workspace_count = snap.workspaces.len();
        self.pane_count = snap.panes.len();
        self.tab_count = snap.tabs.len();
        self.agents = snap.agents.clone();
        // Derive agent rows from panes when agents[] is empty but panes carry status.
        if self.agents.is_empty() {
            self.agents = agents_from_panes(&snap.panes);
        }
        self.snapshot = snap;
        self.live = true;
        self.conn = ConnState::Live;
        self.last_error = None;
        self.snapshot_gen = self.snapshot_gen.saturating_add(1);
        if from_reconnect {
            self.reconnect_count = self.reconnect_count.saturating_add(1);
            self.recent_events.push(format!(
                "resync#{} (snap gen {})",
                self.reconnect_count, self.snapshot_gen
            ));
            if self.recent_events.len() > 12 {
                let drain = self.recent_events.len() - 12;
                self.recent_events.drain(0..drain);
            }
        }
        self.ensure_selection();
        self.status_note = Some(format!(
            "snapshot gen={} · ws={} panes={} agents={} · reconnects={}",
            self.snapshot_gen,
            self.workspace_count,
            self.pane_count,
            self.agents.len(),
            self.reconnect_count
        ));
    }

    pub fn mark_subscribed(&mut self) {
        self.subscribed = true;
        self.conn = ConnState::Live;
        self.live = true;
    }

    pub fn mark_reconnecting(&mut self, reason: impl Into<String>) {
        self.subscribed = false;
        self.conn = ConnState::Reconnecting;
        self.live = false;
        let msg = reason.into();
        self.status_note = Some(format!("reconnecting… · {msg}"));
        self.set_error(msg);
    }

    pub fn mark_unsubscribed(&mut self, reason: impl Into<String>) {
        self.subscribed = false;
        self.set_error(reason);
    }

    /// Apply a wire subscription push (`event` + `data`).
    pub fn apply_event(&mut self, event: &Event) {
        let kind = event.kind_normalized();
        self.event_count = self.event_count.saturating_add(1);
        self.last_event = Some(kind.clone());
        self.recent_events.push(kind.clone());
        if self.recent_events.len() > 12 {
            let drain = self.recent_events.len() - 12;
            self.recent_events.drain(0..drain);
        }

        match kind.as_str() {
            "workspace_created" => {
                if let Some(ws) = event.data.get("workspace") {
                    upsert_by_id(&mut self.snapshot.workspaces, ws, "workspace_id");
                    self.workspace_count = self.snapshot.workspaces.len();
                } else {
                    self.workspace_count = self.workspace_count.saturating_add(1);
                }
            }
            "workspace_closed" => {
                if let Some(id) = event.data.get("workspace_id").and_then(|v| v.as_str()) {
                    self.snapshot
                        .workspaces
                        .retain(|w| w.get("workspace_id").and_then(|v| v.as_str()) != Some(id));
                    self.workspace_count = self.snapshot.workspaces.len();
                    // Drop agents in that workspace if annotated.
                    self.agents.retain(|a| {
                        a.extra
                            .get("workspace_id")
                            .and_then(|v| v.as_str())
                            .map(|w| w != id)
                            .unwrap_or(true)
                    });
                } else {
                    self.workspace_count = self.workspace_count.saturating_sub(1);
                }
            }
            "workspace_updated" | "workspace_renamed" | "workspace_focused" | "workspace_moved" => {
                if let Some(ws) = event.data.get("workspace") {
                    upsert_by_id(&mut self.snapshot.workspaces, ws, "workspace_id");
                    self.workspace_count = self.snapshot.workspaces.len();
                }
            }
            "tab_created" => {
                if let Some(tab) = event.data.get("tab") {
                    upsert_by_id(&mut self.snapshot.tabs, tab, "tab_id");
                    self.tab_count = self.snapshot.tabs.len();
                } else {
                    self.tab_count = self.tab_count.saturating_add(1);
                }
            }
            "tab_closed" => {
                if let Some(id) = event.data.get("tab_id").and_then(|v| v.as_str()) {
                    self.snapshot
                        .tabs
                        .retain(|t| t.get("tab_id").and_then(|v| v.as_str()) != Some(id));
                    self.tab_count = self.snapshot.tabs.len();
                } else {
                    self.tab_count = self.tab_count.saturating_sub(1);
                }
            }
            "tab_updated" | "tab_renamed" | "tab_focused" | "tab_moved" => {
                if let Some(tab) = event.data.get("tab") {
                    upsert_by_id(&mut self.snapshot.tabs, tab, "tab_id");
                    self.tab_count = self.snapshot.tabs.len();
                }
            }
            "pane_created" => {
                if let Some(pane) = event.data.get("pane") {
                    upsert_by_id(&mut self.snapshot.panes, pane, "pane_id");
                    self.pane_count = self.snapshot.panes.len();
                    if let Some(row) = agent_from_pane(pane) {
                        upsert_agent(&mut self.agents, row);
                    }
                } else {
                    self.pane_count = self.pane_count.saturating_add(1);
                }
            }
            "pane_closed" | "pane_exited" => {
                if let Some(id) = event
                    .data
                    .get("pane_id")
                    .or_else(|| event.data.pointer("/pane/pane_id"))
                    .and_then(|v| v.as_str())
                {
                    self.snapshot
                        .panes
                        .retain(|p| p.get("pane_id").and_then(|v| v.as_str()) != Some(id));
                    self.pane_count = self.snapshot.panes.len();
                    self.agents
                        .retain(|a| a.pane_id.as_deref() != Some(id) && a.id != id);
                } else {
                    self.pane_count = self.pane_count.saturating_sub(1);
                }
            }
            "pane_focused" | "pane_moved" | "pane_agent_detected" => {
                if let Some(pane) = event.data.get("pane") {
                    upsert_by_id(&mut self.snapshot.panes, pane, "pane_id");
                    self.pane_count = self.snapshot.panes.len();
                    if let Some(row) = agent_from_pane(pane) {
                        upsert_agent(&mut self.agents, row);
                    }
                }
            }
            "pane_agent_status_changed" | "agent_status_changed" => {
                let pane_id = event
                    .data
                    .get("pane_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let status = event
                    .data
                    .get("agent_status")
                    .or_else(|| event.data.get("state"))
                    .or_else(|| event.data.get("status"))
                    .and_then(|v| v.as_str())
                    .map(parse_state);
                let agent_name = event
                    .data
                    .get("display_agent")
                    .or_else(|| event.data.get("agent"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if let (Some(pid), Some(state)) = (pane_id.clone(), status) {
                    if let Some(a) = self
                        .agents
                        .iter_mut()
                        .find(|a| a.pane_id.as_deref() == Some(pid.as_str()) || a.id == pid)
                    {
                        a.state = state;
                        if agent_name.is_some() {
                            a.name = agent_name;
                        }
                    } else {
                        self.agents.push(AgentSummary {
                            id: pid.clone(),
                            pane_id: Some(pid.clone()),
                            name: agent_name,
                            state,
                            ..Default::default()
                        });
                    }
                    self.resolve_waits_for(&pid, state);
                    // Mirror into pane list if present.
                    for p in &mut self.snapshot.panes {
                        if p.get("pane_id").and_then(|v| v.as_str()) == Some(pid.as_str()) {
                            if let Some(obj) = p.as_object_mut() {
                                obj.insert(
                                    "agent_status".into(),
                                    Value::String(state.as_str().into()),
                                );
                            }
                        }
                    }
                }
            }
            _ => {
                // Forward-compatible: counts/status already updated via last_event.
            }
        }

        self.status_note = Some(format!(
            "live · events={} · last={}",
            self.event_count,
            self.last_event.as_deref().unwrap_or("-")
        ));
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.last_error = Some(msg.into());
    }

    pub fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some(msg.into());
    }

    pub fn set_peek(&mut self, target: impl Into<String>, text: &str) {
        self.peek_target = Some(target.into());
        self.peek_lines = text.lines().map(|l| l.to_string()).collect();
        self.peek_busy = false;
    }

    pub fn filtered_agents(&self) -> Vec<(usize, &AgentSummary)> {
        self.agents
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                if let Some(st) = self.filter.state {
                    if a.state != st {
                        return false;
                    }
                }
                if !self.filter.query.is_empty() {
                    let q = self.filter.query.to_lowercase();
                    let name = a.name.as_deref().unwrap_or("");
                    if !a.id.to_lowercase().contains(&q) && !name.to_lowercase().contains(&q) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Selected agent in the **unfiltered** agents list.
    pub fn selected_agent(&self) -> Option<&AgentSummary> {
        self.selected.and_then(|i| self.agents.get(i))
    }

    pub fn selected_target(&self) -> Option<String> {
        self.selected_agent()
            .map(|a| a.pane_id.clone().unwrap_or_else(|| a.id.clone()))
    }

    /// Move selection within the filtered view; stores index into full `agents`.
    pub fn select_delta(&mut self, delta: i32) {
        let rows = self.filtered_agents();
        if rows.is_empty() {
            self.selected = None;
            return;
        }
        let cur_pos = self
            .selected
            .and_then(|sel| rows.iter().position(|(i, _)| *i == sel))
            .unwrap_or(0);
        let n = rows.len() as i32;
        let next = (cur_pos as i32 + delta).rem_euclid(n) as usize;
        self.selected = Some(rows[next].0);
    }

    pub fn ensure_selection(&mut self) {
        if self.agents.is_empty() {
            self.selected = None;
            return;
        }
        if self
            .selected
            .map(|i| i >= self.agents.len())
            .unwrap_or(true)
        {
            // Prefer first filtered row.
            let rows = self.filtered_agents();
            self.selected = rows.first().map(|(i, _)| *i).or(Some(0));
        }
    }

    pub fn set_filter_state(&mut self, state: Option<AgentState>) {
        self.filter.state = state;
        self.ensure_selection();
    }

    pub fn cycle_filter_state(&mut self) {
        self.filter.state = match self.filter.state {
            None => Some(AgentState::Idle),
            Some(AgentState::Idle) => Some(AgentState::Working),
            Some(AgentState::Working) => Some(AgentState::Blocked),
            Some(AgentState::Blocked) => Some(AgentState::Done),
            Some(AgentState::Done) => Some(AgentState::Unknown),
            Some(AgentState::Unknown) => None,
        };
        self.ensure_selection();
    }

    pub fn state_counts(&self) -> [(AgentState, usize); 5] {
        use AgentState::*;
        let mut c = [
            (Idle, 0),
            (Working, 0),
            (Blocked, 0),
            (Done, 0),
            (Unknown, 0),
        ];
        for a in &self.agents {
            match a.state {
                Idle => c[0].1 += 1,
                Working => c[1].1 += 1,
                Blocked => c[2].1 += 1,
                Done => c[3].1 += 1,
                Unknown => c[4].1 += 1,
            }
        }
        c
    }

    /// Merge agents from `agent.list` / pane-derived JSON rows.
    pub fn merge_agent_values(&mut self, rows: &[Value]) {
        for row in rows {
            if let Some(summary) = agent_from_list_row(row) {
                upsert_agent(&mut self.agents, summary);
            }
        }
        self.ensure_selection();
    }

    pub fn arm_wait(&mut self, target: String, want: AgentState) {
        // Resolve immediately if already satisfied.
        let current = self
            .agents
            .iter()
            .find(|a| a.pane_id.as_deref() == Some(target.as_str()) || a.id == target)
            .map(|a| a.state);
        let resolved = current == Some(want);
        self.waits.retain(|w| w.target != target);
        self.waits.push(WaitBadge {
            message: if resolved {
                format!("{target} already {want}", want = want.as_str())
            } else {
                format!("waiting {target} → {}", want.as_str())
            },
            target,
            want,
            armed: !resolved,
            resolved,
        });
        if resolved {
            self.set_toast("wait already satisfied");
        }
    }

    /// Called from event reduce when agent status changes.
    fn resolve_waits_for(&mut self, target: &str, state: AgentState) {
        for w in &mut self.waits {
            if w.armed && !w.resolved && w.target == target && w.want == state {
                w.resolved = true;
                w.armed = false;
                w.message = format!("wait ok · {target} is {}", state.as_str());
                self.toast = Some(w.message.clone());
            }
        }
    }
}

fn upsert_by_id(list: &mut Vec<Value>, item: &Value, id_key: &str) {
    let Some(id) = item.get(id_key).and_then(|v| v.as_str()) else {
        list.push(item.clone());
        return;
    };
    if let Some(slot) = list
        .iter_mut()
        .find(|v| v.get(id_key).and_then(|x| x.as_str()) == Some(id))
    {
        *slot = item.clone();
    } else {
        list.push(item.clone());
    }
}

fn upsert_agent(agents: &mut Vec<AgentSummary>, row: AgentSummary) {
    if let Some(a) = agents
        .iter_mut()
        .find(|a| a.id == row.id || (row.pane_id.is_some() && a.pane_id == row.pane_id))
    {
        *a = row;
    } else {
        agents.push(row);
    }
}

fn agents_from_panes(panes: &[Value]) -> Vec<AgentSummary> {
    panes.iter().filter_map(agent_from_pane).collect()
}

fn agent_from_list_row(row: &Value) -> Option<AgentSummary> {
    // agent.list items often look like AgentInfo (pane_id, agent_status, …)
    if row.get("pane_id").is_some() {
        return agent_from_pane(row);
    }
    let id = row
        .get("id")
        .or_else(|| row.get("target"))
        .or_else(|| row.get("name"))
        .and_then(|v| v.as_str())?
        .to_string();
    let state = row
        .get("agent_status")
        .or_else(|| row.get("state"))
        .and_then(|v| v.as_str())
        .map(parse_state)
        .unwrap_or(AgentState::Unknown);
    let name = row
        .get("display_agent")
        .or_else(|| row.get("agent"))
        .or_else(|| row.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some(AgentSummary {
        id: id.clone(),
        name,
        state,
        pane_id: row
            .get("pane_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or(Some(id)),
        cwd: row
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        extra: Default::default(),
    })
}

fn agent_from_pane(pane: &Value) -> Option<AgentSummary> {
    let pane_id = pane.get("pane_id")?.as_str()?.to_string();
    let status = pane
        .get("agent_status")
        .and_then(|v| v.as_str())
        .map(parse_state)
        .unwrap_or(AgentState::Unknown);
    // Only surface panes that look agent-like or always surface all panes as rows.
    // Board shows all panes for observability of parallel work.
    let name = pane
        .get("display_agent")
        .or_else(|| pane.get("agent"))
        .or_else(|| pane.get("label"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let cwd = pane
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mut extra = serde_json::Map::new();
    if let Some(ws) = pane.get("workspace_id").cloned() {
        extra.insert("workspace_id".into(), ws);
    }
    Some(AgentSummary {
        id: pane_id.clone(),
        name,
        state: status,
        pane_id: Some(pane_id),
        cwd,
        extra,
    })
}

fn parse_state(s: &str) -> AgentState {
    match s.to_lowercase().as_str() {
        "idle" => AgentState::Idle,
        "working" | "running" => AgentState::Working,
        "blocked" => AgentState::Blocked,
        "done" | "completed" => AgentState::Done,
        _ => AgentState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn filter_by_state() {
        let mut s = Store {
            agents: vec![
                AgentSummary {
                    id: "1".into(),
                    state: AgentState::Idle,
                    ..Default::default()
                },
                AgentSummary {
                    id: "2".into(),
                    state: AgentState::Working,
                    ..Default::default()
                },
            ],
            filter: BoardFilter {
                state: Some(AgentState::Working),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(s.filtered_agents().len(), 1);
        let _ = &mut s;
    }

    #[test]
    fn event_updates_state() {
        let mut s = Store {
            agents: vec![AgentSummary {
                id: "w1:p1".into(),
                pane_id: Some("w1:p1".into()),
                state: AgentState::Working,
                ..Default::default()
            }],
            ..Default::default()
        };
        let ev = Event {
            event: "pane_agent_status_changed".into(),
            data: json!({"pane_id":"w1:p1","agent_status":"done"}),
            extra: Default::default(),
        };
        s.apply_event(&ev);
        assert_eq!(s.agents[0].state, AgentState::Done);
        assert_eq!(s.event_count, 1);
    }

    #[test]
    fn workspace_created_bumps_count() {
        let mut s = Store::default();
        let ev = Event {
            event: "workspace_created".into(),
            data: json!({
                "type": "workspace_created",
                "workspace": {"workspace_id": "w9", "label": "x"}
            }),
            extra: Default::default(),
        };
        s.apply_event(&ev);
        assert_eq!(s.workspace_count, 1);
        assert_eq!(s.snapshot.workspaces.len(), 1);
    }

    #[test]
    fn resnapshot_replaces_and_bumps_metrics() {
        let mut s = Store::default();
        s.apply_snapshot(SessionSnapshot {
            workspaces: vec![json!({"workspace_id":"w1"})],
            panes: vec![json!({"pane_id":"w1:p1","agent_status":"idle"})],
            ..Default::default()
        });
        assert_eq!(s.snapshot_gen, 1);
        assert_eq!(s.workspace_count, 1);
        assert_eq!(s.reconnect_count, 0);

        // Stale event-era agent should be wiped by full replace.
        s.agents.push(AgentSummary {
            id: "stale".into(),
            ..Default::default()
        });

        s.apply_resnapshot(SessionSnapshot {
            workspaces: vec![json!({"workspace_id":"w2"}), json!({"workspace_id":"w3"})],
            panes: vec![json!({"pane_id":"w2:p1","agent_status":"working"})],
            ..Default::default()
        });
        assert_eq!(s.snapshot_gen, 2);
        assert_eq!(s.reconnect_count, 1);
        assert_eq!(s.workspace_count, 2);
        assert!(s.agents.iter().all(|a| a.id != "stale"));
        assert_eq!(s.agents.len(), 1);
        assert_eq!(s.agents[0].state, AgentState::Working);
    }

    #[test]
    fn select_delta_moves_within_filter() {
        let mut s = Store {
            agents: vec![
                AgentSummary {
                    id: "a".into(),
                    state: AgentState::Idle,
                    pane_id: Some("a".into()),
                    ..Default::default()
                },
                AgentSummary {
                    id: "b".into(),
                    state: AgentState::Working,
                    pane_id: Some("b".into()),
                    ..Default::default()
                },
                AgentSummary {
                    id: "c".into(),
                    state: AgentState::Working,
                    pane_id: Some("c".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        s.set_filter_state(Some(AgentState::Working));
        s.ensure_selection();
        let first = s.selected_target().unwrap();
        s.select_delta(1);
        let second = s.selected_target().unwrap();
        assert_ne!(first, second);
        assert!(s.selected_agent().unwrap().state == AgentState::Working);
    }

    #[test]
    fn wait_resolves_on_status_event() {
        let mut s = Store {
            agents: vec![AgentSummary {
                id: "w1:p1".into(),
                pane_id: Some("w1:p1".into()),
                state: AgentState::Working,
                ..Default::default()
            }],
            ..Default::default()
        };
        s.arm_wait("w1:p1".into(), AgentState::Done);
        assert!(s.waits.iter().any(|w| w.armed && !w.resolved));
        s.apply_event(&Event {
            event: "pane_agent_status_changed".into(),
            data: json!({"pane_id":"w1:p1","agent_status":"done"}),
            extra: Default::default(),
        });
        assert!(s.waits.iter().any(|w| w.resolved && !w.armed));
        assert_eq!(s.agents[0].state, AgentState::Done);
    }

    #[test]
    fn set_peek_splits_lines() {
        let mut s = Store::default();
        s.set_peek("w1:p1", "one\ntwo\nthree");
        assert_eq!(s.peek_target.as_deref(), Some("w1:p1"));
        assert_eq!(s.peek_lines.len(), 3);
        assert!(!s.peek_busy);
    }

    #[test]
    fn dotted_event_kind_normalizes() {
        let mut s = Store::default();
        s.apply_event(&Event {
            event: "workspace.created".into(),
            data: json!({"workspace":{"workspace_id":"wx"}}),
            extra: Default::default(),
        });
        assert_eq!(s.workspace_count, 1);
        assert_eq!(s.last_event.as_deref(), Some("workspace_created"));
    }
}
