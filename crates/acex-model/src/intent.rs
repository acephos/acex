//! UI → runtime intents (Phase 1 control plane).

use herdr_types::AgentState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartPreset {
    pub id: String,
    pub name: String,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutPreset {
    pub id: String,
    pub name: String,
    pub root: LayoutNode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayoutNode {
    Pane {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        env: BTreeMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pane_id: Option<String>,
    },
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirection {
    Right,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachTarget {
    SelectedAgent,
    Agent(String),
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCreateSpec {
    pub branch: Option<String>,
    pub path: Option<String>,
    pub base: Option<String>,
    pub label: Option<String>,
    pub cwd: Option<String>,
    pub workspace_id: Option<String>,
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeOpenSpec {
    pub branch: Option<String>,
    pub path: Option<String>,
    pub label: Option<String>,
    pub cwd: Option<String>,
    pub workspace_id: Option<String>,
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRemoveSpec {
    pub workspace_id: String,
    pub force: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFocusSpec {
    pub workspace_id: String,
}

/// Commands raised by the UI; executed on the async Herdr worker.
#[derive(Debug, Clone)]
pub enum Intent {
    FocusSelected,
    FocusTarget(String),
    PeekSelected,
    PeekTarget(String),
    SendSelected {
        text: String,
    },
    RunPaneSelected {
        command: String,
    },
    StartAgent {
        name: String,
        argv: Vec<String>,
        cwd: Option<String>,
    },
    WaitSelected {
        status: AgentState,
    },
    OpenZed {
        path: Option<String>,
        mode: ZedOpenMode,
    },
    DiffZed {
        old: String,
        new: String,
    },
    ApplyLayout(LayoutPreset),
    Attach {
        target: AttachTarget,
    },
    WorkspaceList,
    WorkspaceFocus(WorkspaceFocusSpec),
    WorktreeList,
    WorktreeCreate(WorktreeCreateSpec),
    WorktreeOpen(WorktreeOpenSpec),
    WorktreeRemove(WorktreeRemoveSpec),
    Resnapshot,
    Notify {
        title: String,
        body: Option<String>,
    },
    RefreshAgents,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ZedOpenMode {
    #[default]
    Default,
    NewWindow,
    AddToWindow,
}
