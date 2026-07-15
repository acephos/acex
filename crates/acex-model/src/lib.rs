//! In-memory control-plane model.
//!
//! Herdr is authority. This store is a reduce of snapshot + events.

mod intent;

pub use herdr_types::{AgentState, AgentSummary, Event, SessionSnapshot};
pub use intent::{
    AttachTarget, Intent, LayoutNode, LayoutPreset, SplitDirection, StartPreset,
    WorkspaceFocusSpec, WorktreeCreateSpec, WorktreeOpenSpec, WorktreeRemoveSpec, ZedOpenMode,
};

use serde_json::Value;
use std::collections::BTreeSet;

pub const DEFAULT_WAIT_TIMEOUT_MS: u64 = 30_000;

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

/// Read-only cwd/path target surfaced from Herdr snapshots for handoffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathTarget<'a> {
    pub source: &'static str,
    pub id: Option<&'a str>,
    pub label: Option<&'a str>,
    pub path: &'a str,
    pub selected: bool,
}
/// Workspace focus/scoping target surfaced from Herdr snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceTarget<'a> {
    pub id: &'a str,
    pub label: Option<&'a str>,
    pub focused: bool,
    pub scoped: bool,
}

pub const AGENT_ACTIVITY_STALE_EVENTS: u64 = 10;
pub const AGENT_ACTIVITY_DISPLAY_CAP_EVENTS: u64 = 99;

/// Last source signal observed for an agent row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentActivity {
    pub target: String,
    pub last_event_count: u64,
    pub last_signal: String,
}

/// Event-age view for board/detail activity badges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentActivityAge<'a> {
    pub events_since: u64,
    pub stale: bool,
    pub signal: &'a str,
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
    /// Per-agent last signal, derived only from snapshots/list rows/events we already receive.
    pub agent_activity: Vec<AgentActivity>,
}

/// Non-blocking wait tracker for a pane/agent.
#[derive(Debug, Clone)]
pub struct WaitBadge {
    pub target: String,
    pub want: AgentState,
    pub armed: bool,
    pub resolved: bool,
    pub expired: bool,
    pub expires_at_ms: Option<u64>,
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
        self.reset_agent_activity(if from_reconnect { "resync" } else { "snapshot" });
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
                    // Drop agents in that workspace if annotated, and clear stale board scope.
                    self.agents.retain(|a| agent_workspace_id(a) != Some(id));
                    self.prune_agent_activity();
                    if self.filter.workspace.as_deref() == Some(id) {
                        self.filter.workspace = None;
                    }
                } else {
                    self.workspace_count = self.workspace_count.saturating_sub(1);
                }
            }
            "workspace_updated" | "workspace_renamed" | "workspace_moved" => {
                if let Some(ws) = event.data.get("workspace") {
                    upsert_by_id(&mut self.snapshot.workspaces, ws, "workspace_id");
                    self.workspace_count = self.snapshot.workspaces.len();
                }
            }
            "workspace_focused" => {
                if let Some(ws) = event.data.get("workspace") {
                    upsert_by_id(&mut self.snapshot.workspaces, ws, "workspace_id");
                    self.workspace_count = self.snapshot.workspaces.len();
                }
                if let Some(id) = event.data.get("workspace_id").and_then(|v| v.as_str()) {
                    mark_workspace_focused(&mut self.snapshot.workspaces, id);
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
                        self.note_agent_row_activity(&row, kind.as_str());
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
                    self.prune_agent_activity();
                } else {
                    self.pane_count = self.pane_count.saturating_sub(1);
                }
            }
            "pane_focused" | "pane_moved" | "pane_agent_detected" => {
                if let Some(pane) = event.data.get("pane") {
                    upsert_by_id(&mut self.snapshot.panes, pane, "pane_id");
                    self.pane_count = self.snapshot.panes.len();
                    if let Some(row) = agent_from_pane(pane) {
                        self.note_agent_row_activity(&row, kind.as_str());
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
                    self.note_agent_activity(&pid, kind.as_str());
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
    pub fn apply_workspace_values(&mut self, rows: Vec<Value>) {
        self.snapshot.workspaces = rows;
        self.workspace_count = self.snapshot.workspaces.len();
        if let Some(current) = self.filter.workspace.as_deref() {
            if !workspace_exists(&self.snapshot.workspaces, current) {
                self.filter.workspace = None;
            }
        }
        self.ensure_selection();
    }

    pub fn path_targets(&self) -> Vec<PathTarget<'_>> {
        let mut rows = Vec::new();
        let mut seen = BTreeSet::new();

        if let Some(index) = self.selected {
            if let Some(agent) = self.agents.get(index) {
                push_agent_path(&mut rows, &mut seen, agent, true);
            }
        }
        for (index, agent) in self.agents.iter().enumerate() {
            if self.selected == Some(index) {
                continue;
            }
            push_agent_path(&mut rows, &mut seen, agent, false);
        }
        for workspace in &self.snapshot.workspaces {
            push_json_path(
                &mut rows,
                &mut seen,
                "workspace",
                workspace,
                &["workspace_id", "id"],
                &["label", "name"],
            );
        }
        for pane in &self.snapshot.panes {
            push_json_path(
                &mut rows,
                &mut seen,
                "pane",
                pane,
                &["pane_id", "id"],
                &["label", "display_agent", "agent"],
            );
        }

        rows
    }
    pub fn workspace_targets(&self) -> Vec<WorkspaceTarget<'_>> {
        let scoped = self.filter.workspace.as_deref();
        self.snapshot
            .workspaces
            .iter()
            .filter_map(|workspace| {
                let id = value_str(workspace, &["workspace_id", "id"])?;
                Some(WorkspaceTarget {
                    id,
                    label: value_str(workspace, &["label", "name"]),
                    focused: workspace
                        .get("focused")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    scoped: scoped == Some(id),
                })
            })
            .collect()
    }

    pub fn workspace_scope_label(&self) -> String {
        let Some(workspace_id) = self.filter.workspace.as_deref() else {
            return "all".into();
        };
        self.workspace_targets()
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .and_then(|workspace| workspace.label)
            .unwrap_or(workspace_id)
            .to_string()
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
                if let Some(workspace) = self.filter.workspace.as_deref() {
                    if agent_workspace_id(a) != Some(workspace) {
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

    pub fn agent_activity_age(&self, agent: &AgentSummary) -> Option<AgentActivityAge<'_>> {
        let target = agent_target(agent);
        let activity = self.agent_activity.iter().find(|activity| {
            activity.target == target
                || activity.target == agent.id
                || agent.pane_id.as_deref() == Some(activity.target.as_str())
        })?;
        let events_since = self.event_count.saturating_sub(activity.last_event_count);
        Some(AgentActivityAge {
            events_since,
            stale: events_since >= AGENT_ACTIVITY_STALE_EVENTS,
            signal: activity.last_signal.as_str(),
        })
    }

    fn reset_agent_activity(&mut self, signal: &str) {
        self.agent_activity.clear();
        let targets = self
            .agents
            .iter()
            .map(agent_target)
            .map(str::to_string)
            .collect::<Vec<_>>();
        for target in targets {
            self.note_agent_activity(&target, signal);
        }
    }

    fn note_agent_row_activity(&mut self, agent: &AgentSummary, signal: &str) {
        let target = agent_target(agent).to_string();
        self.note_agent_activity(&target, signal);
    }

    fn note_agent_activity(&mut self, target: &str, signal: &str) {
        if target.trim().is_empty() {
            return;
        }
        if let Some(activity) = self
            .agent_activity
            .iter_mut()
            .find(|activity| activity.target == target)
        {
            activity.last_event_count = self.event_count;
            activity.last_signal.clear();
            activity.last_signal.push_str(signal);
            return;
        }
        self.agent_activity.push(AgentActivity {
            target: target.to_string(),
            last_event_count: self.event_count,
            last_signal: signal.to_string(),
        });
    }

    fn prune_agent_activity(&mut self) {
        let mut live = BTreeSet::new();
        for agent in &self.agents {
            live.insert(agent.id.clone());
            if let Some(pane_id) = &agent.pane_id {
                live.insert(pane_id.clone());
            }
        }
        self.agent_activity
            .retain(|activity| live.contains(&activity.target));
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
        let rows = self.filtered_agents();
        if rows.is_empty() {
            self.selected = None;
            return;
        }
        if self
            .selected
            .is_some_and(|selected| rows.iter().any(|(index, _)| *index == selected))
        {
            return;
        }
        self.selected = Some(rows[0].0);
    }

    pub fn set_filter_state(&mut self, state: Option<AgentState>) {
        self.filter.state = state;
        self.ensure_selection();
    }
    pub fn set_workspace_filter(&mut self, workspace: Option<String>) {
        self.filter.workspace = workspace.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });
        self.ensure_selection();
    }

    pub fn cycle_workspace_filter(&mut self) {
        let ids = self
            .snapshot
            .workspaces
            .iter()
            .filter_map(|workspace| value_str(workspace, &["workspace_id", "id"]))
            .collect::<Vec<_>>();
        self.filter.workspace = match self.filter.workspace.as_deref() {
            None => ids.first().map(|id| (*id).to_string()),
            Some(current) => ids
                .iter()
                .position(|id| *id == current)
                .and_then(|index| ids.get(index + 1))
                .map(|id| (*id).to_string()),
        };
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
                self.note_agent_row_activity(&summary, "agent.list");
                upsert_agent(&mut self.agents, summary);
            }
        }
        self.prune_agent_activity();
        self.ensure_selection();
    }

    pub fn arm_wait(&mut self, target: String, want: AgentState) {
        self.arm_wait_at(target, want, 0, DEFAULT_WAIT_TIMEOUT_MS);
    }

    pub fn arm_wait_at(&mut self, target: String, want: AgentState, now_ms: u64, timeout_ms: u64) {
        // Resolve immediately if already satisfied.
        let current = self
            .agents
            .iter()
            .find(|a| a.pane_id.as_deref() == Some(target.as_str()) || a.id == target)
            .map(|a| a.state);
        let resolved = current == Some(want);
        let expires_at_ms = (!resolved).then_some(now_ms.saturating_add(timeout_ms));
        self.waits.retain(|w| w.target != target);
        self.waits.push(WaitBadge {
            message: if resolved {
                format!("{target} already {want}", want = want.as_str())
            } else {
                format!(
                    "waiting {target} → {want} · expires in {secs}s",
                    want = want.as_str(),
                    secs = timeout_ms / 1_000
                )
            },
            target,
            want,
            armed: !resolved,
            resolved,
            expired: false,
            expires_at_ms,
        });
        if resolved {
            self.set_toast("wait already satisfied");
        }
    }

    pub fn expire_waits(&mut self, now_ms: u64) {
        for w in &mut self.waits {
            if w.armed
                && !w.resolved
                && !w.expired
                && w.expires_at_ms.is_some_and(|expires| now_ms >= expires)
            {
                w.armed = false;
                w.expired = true;
                w.message = format!("wait expired · {} not {}", w.target, w.want.as_str());
                self.toast = Some(w.message.clone());
            }
        }
    }

    /// Called from event reduce when agent status changes.
    fn resolve_waits_for(&mut self, target: &str, state: AgentState) {
        for w in &mut self.waits {
            if w.armed && !w.resolved && !w.expired && w.target == target && w.want == state {
                w.resolved = true;
                w.armed = false;
                w.message = format!("wait ok · {target} is {}", state.as_str());
                self.toast = Some(w.message.clone());
            }
        }
    }
}

fn push_agent_path<'a>(
    rows: &mut Vec<PathTarget<'a>>,
    seen: &mut BTreeSet<&'a str>,
    agent: &'a AgentSummary,
    selected: bool,
) {
    let Some(path) = non_blank_str(agent.cwd.as_deref()) else {
        return;
    };
    if !seen.insert(path) {
        return;
    }
    rows.push(PathTarget {
        source: "agent",
        id: Some(agent.pane_id.as_deref().unwrap_or(agent.id.as_str())),
        label: agent.name.as_deref(),
        path,
        selected,
    });
}

fn push_json_path<'a>(
    rows: &mut Vec<PathTarget<'a>>,
    seen: &mut BTreeSet<&'a str>,
    source: &'static str,
    item: &'a Value,
    id_keys: &[&str],
    label_keys: &[&str],
) {
    let Some(path) = value_str(item, &["cwd", "path"]) else {
        return;
    };
    if !seen.insert(path) {
        return;
    }
    rows.push(PathTarget {
        source,
        id: value_str(item, id_keys),
        label: value_str(item, label_keys),
        path,
        selected: false,
    });
}

fn value_str<'a>(item: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|key| item.get(*key).and_then(|v| v.as_str()))
        .find(|s| !s.trim().is_empty())
}

fn non_blank_str(value: Option<&str>) -> Option<&str> {
    value.filter(|s| !s.trim().is_empty())
}

fn agent_workspace_id(agent: &AgentSummary) -> Option<&str> {
    agent
        .extra
        .get("workspace_id")
        .and_then(|v| v.as_str())
        .and_then(|s| (!s.trim().is_empty()).then_some(s))
}

fn agent_target(agent: &AgentSummary) -> &str {
    agent.pane_id.as_deref().unwrap_or(agent.id.as_str())
}

fn workspace_exists(workspaces: &[Value], id: &str) -> bool {
    workspaces
        .iter()
        .filter_map(|workspace| value_str(workspace, &["workspace_id", "id"]))
        .any(|workspace_id| workspace_id == id)
}

fn mark_workspace_focused(workspaces: &mut [Value], focused_id: &str) {
    for workspace in workspaces {
        let is_focused = value_str(workspace, &["workspace_id", "id"]) == Some(focused_id);
        if let Some(object) = workspace.as_object_mut() {
            object.insert("focused".into(), Value::Bool(is_focused));
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
    fn activity_age_tracks_snapshot_and_stale_event_distance() {
        let mut s = Store::default();
        s.apply_snapshot(SessionSnapshot {
            panes: vec![json!({"pane_id":"w1:p1","agent_status":"idle"})],
            ..Default::default()
        });

        let age = s.agent_activity_age(&s.agents[0]).expect("activity age");
        assert_eq!(age.events_since, 0);
        assert_eq!(age.signal, "snapshot");
        assert!(!age.stale);

        for n in 0..AGENT_ACTIVITY_STALE_EVENTS {
            s.apply_event(&Event {
                event: "workspace_created".into(),
                data: json!({"workspace": {"workspace_id": format!("ws-{n}")}}),
                extra: Default::default(),
            });
        }

        let age = s.agent_activity_age(&s.agents[0]).expect("activity age");
        assert_eq!(age.events_since, AGENT_ACTIVITY_STALE_EVENTS);
        assert!(age.stale);
    }

    #[test]
    fn status_event_refreshes_agent_activity_age() {
        let mut s = Store {
            agents: vec![AgentSummary {
                id: "w1:p1".into(),
                pane_id: Some("w1:p1".into()),
                state: AgentState::Working,
                ..Default::default()
            }],
            ..Default::default()
        };
        s.reset_agent_activity("seed");
        for n in 0..3 {
            s.apply_event(&Event {
                event: "workspace_created".into(),
                data: json!({"workspace": {"workspace_id": format!("ws-{n}")}}),
                extra: Default::default(),
            });
        }
        assert_eq!(s.agent_activity_age(&s.agents[0]).unwrap().events_since, 3);

        s.apply_event(&Event {
            event: "pane_agent_status_changed".into(),
            data: json!({"pane_id":"w1:p1","agent_status":"done"}),
            extra: Default::default(),
        });

        let age = s.agent_activity_age(&s.agents[0]).expect("activity age");
        assert_eq!(age.events_since, 0);
        assert_eq!(age.signal, "pane_agent_status_changed");
        assert!(!age.stale);
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
    fn workspace_filter_scopes_agents_and_selection() {
        let mut agent_a = AgentSummary {
            id: "a".into(),
            pane_id: Some("pane-a".into()),
            state: AgentState::Working,
            ..Default::default()
        };
        agent_a.extra.insert("workspace_id".into(), json!("ws-a"));
        let mut agent_b = AgentSummary {
            id: "b".into(),
            pane_id: Some("pane-b".into()),
            state: AgentState::Working,
            ..Default::default()
        };
        agent_b.extra.insert("workspace_id".into(), json!("ws-b"));
        let mut s = Store {
            selected: Some(0),
            agents: vec![agent_a, agent_b],
            snapshot: SessionSnapshot {
                workspaces: vec![
                    json!({"workspace_id":"ws-a", "label":"Alpha"}),
                    json!({"workspace_id":"ws-b", "label":"Beta"}),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        s.set_workspace_filter(Some("ws-b".into()));

        let rows = s.filtered_agents();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1.id, "b");
        assert_eq!(s.selected_target().as_deref(), Some("pane-b"));
        assert_eq!(s.workspace_scope_label(), "Beta");
    }

    #[test]
    fn workspace_focused_event_marks_snapshot_without_changing_board_scope() {
        let mut s = Store {
            snapshot: SessionSnapshot {
                workspaces: vec![
                    json!({"workspace_id":"ws-a", "label":"Alpha", "focused":true}),
                    json!({"workspace_id":"ws-b", "label":"Beta", "focused":false}),
                ],
                ..Default::default()
            },
            filter: BoardFilter {
                workspace: Some("ws-a".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        s.apply_event(&Event {
            event: "workspace_focused".into(),
            data: json!({"workspace_id":"ws-b"}),
            extra: Default::default(),
        });

        assert_eq!(s.filter.workspace.as_deref(), Some("ws-a"));
        assert_eq!(s.snapshot.workspaces.len(), 2);
        let targets = s.workspace_targets();
        assert!(targets
            .iter()
            .any(|workspace| workspace.id == "ws-b" && workspace.focused));
        assert!(targets
            .iter()
            .any(|workspace| workspace.id == "ws-a" && workspace.scoped));
    }

    #[test]
    fn workspace_list_replaces_source_rows_and_clears_stale_scope() {
        let mut s = Store::default();
        s.set_workspace_filter(Some("stale".into()));

        s.apply_workspace_values(vec![json!({"workspace_id":"ws-a", "label":"Alpha"})]);

        assert_eq!(s.workspace_count, 1);
        assert_eq!(s.filter.workspace, None);
        assert_eq!(s.workspace_targets()[0].id, "ws-a");
    }

    #[test]
    fn path_targets_prefer_selected_agent_and_dedupe_snapshot_paths() {
        let s = Store {
            selected: Some(1),
            agents: vec![
                AgentSummary {
                    id: "agent-a".into(),
                    pane_id: Some("pane-a".into()),
                    cwd: Some("/repo/a".into()),
                    ..Default::default()
                },
                AgentSummary {
                    id: "agent-b".into(),
                    name: Some("builder".into()),
                    pane_id: Some("pane-b".into()),
                    cwd: Some("/repo/b".into()),
                    ..Default::default()
                },
            ],
            snapshot: SessionSnapshot {
                workspaces: vec![
                    json!({"workspace_id":"ws-b", "label":"same", "cwd":"/repo/b"}),
                    json!({"workspace_id":"ws-c", "label":"other", "path":"/repo/c"}),
                ],
                panes: vec![json!({"pane_id":"pane-d", "label":"logs", "cwd":"/repo/d"})],
                ..Default::default()
            },
            ..Default::default()
        };

        let rows = s.path_targets();

        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].source, "agent");
        assert_eq!(rows[0].id, Some("pane-b"));
        assert_eq!(rows[0].label, Some("builder"));
        assert_eq!(rows[0].path, "/repo/b");
        assert!(rows[0].selected);
        assert_eq!(rows[1].path, "/repo/a");
        assert_eq!(rows[2].source, "workspace");
        assert_eq!(rows[2].id, Some("ws-c"));
        assert_eq!(rows[2].path, "/repo/c");
        assert_eq!(rows[3].source, "pane");
        assert_eq!(rows[3].id, Some("pane-d"));
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
        s.arm_wait_at("w1:p1".into(), AgentState::Done, 0, DEFAULT_WAIT_TIMEOUT_MS);
        assert!(s.waits.iter().any(|w| w.armed && !w.resolved && !w.expired));
        s.apply_event(&Event {
            event: "pane_agent_status_changed".into(),
            data: json!({"pane_id":"w1:p1","agent_status":"done"}),
            extra: Default::default(),
        });
        assert!(s.waits.iter().any(|w| w.resolved && !w.armed && !w.expired));
        assert_eq!(s.agents[0].state, AgentState::Done);
    }

    #[test]
    fn wait_timeout_before_event_stays_expired() {
        let mut s = Store {
            agents: vec![AgentSummary {
                id: "w1:p1".into(),
                pane_id: Some("w1:p1".into()),
                state: AgentState::Working,
                ..Default::default()
            }],
            ..Default::default()
        };
        s.arm_wait_at("w1:p1".into(), AgentState::Done, 100, 50);

        s.expire_waits(151);

        assert!(s.waits.iter().any(|w| w.expired && !w.armed && !w.resolved));
        s.apply_event(&Event {
            event: "pane_agent_status_changed".into(),
            data: json!({"pane_id":"w1:p1","agent_status":"done"}),
            extra: Default::default(),
        });
        assert!(s.waits.iter().any(|w| w.expired && !w.resolved));
        assert_eq!(s.agents[0].state, AgentState::Done);
    }

    #[test]
    fn wait_event_before_timeout_stays_resolved() {
        let mut s = Store {
            agents: vec![AgentSummary {
                id: "w1:p1".into(),
                pane_id: Some("w1:p1".into()),
                state: AgentState::Working,
                ..Default::default()
            }],
            ..Default::default()
        };
        s.arm_wait_at("w1:p1".into(), AgentState::Done, 100, 50);

        s.apply_event(&Event {
            event: "pane_agent_status_changed".into(),
            data: json!({"pane_id":"w1:p1","agent_status":"done"}),
            extra: Default::default(),
        });
        s.expire_waits(151);

        assert!(s.waits.iter().any(|w| w.resolved && !w.armed && !w.expired));
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
    fn set_peek_preserves_ansi_escape_sequences() {
        let mut s = Store::default();
        s.set_peek("w1:p1", "\u{1b}[31mred\u{1b}[0m\nplain");

        assert_eq!(s.peek_lines[0], "\u{1b}[31mred\u{1b}[0m");
        assert_eq!(s.peek_lines[1], "plain");
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
