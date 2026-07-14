//! UI → runtime intents (Phase 1 control plane).

use herdr_types::AgentState;

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
    AttachSelected,
    AttachSession,
    WorktreeList,
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
