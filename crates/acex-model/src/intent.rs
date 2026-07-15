//! UI → runtime intents (Phase 1 control plane).

use herdr_types::AgentState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartPreset {
    pub id: String,
    pub name: String,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
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
    Attach {
        target: AttachTarget,
    },
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
