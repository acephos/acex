//! Command palette registry (F14 / F29 spine).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteAction {
    Focus,
    Peek,
    Send,
    Start,
    WaitDone,
    WaitBlocked,
    OpenZed,
    OpenZedNew,
    Attach,
    AttachSession,
    Worktrees,
    Resnapshot,
    RefreshAgents,
    Notify,
}

impl PaletteAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Focus => "Focus selected agent",
            Self::Peek => "Peek selected output",
            Self::Send => "Send text to selected",
            Self::Start => "Start agent…",
            Self::WaitDone => "Wait until done",
            Self::WaitBlocked => "Wait until blocked",
            Self::OpenZed => "Open in Zed",
            Self::OpenZedNew => "Open in Zed (new window)",
            Self::Attach => "Attach selected in Herdr",
            Self::AttachSession => "Attach Herdr session TUI",
            Self::Worktrees => "List worktrees",
            Self::Resnapshot => "Resnapshot session",
            Self::RefreshAgents => "Refresh agent list",
            Self::Notify => "Show Herdr notification…",
        }
    }

    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Focus => &["focus", "f"],
            Self::Peek => &["peek", "read", "output", "r"],
            Self::Send => &["send", "inject", "s"],
            Self::Start => &["start", "launch", "run", "agent"],
            Self::WaitDone => &["wait", "done"],
            Self::WaitBlocked => &["wait", "blocked"],
            Self::OpenZed => &["zed", "edit", "open"],
            Self::OpenZedNew => &["zed", "new", "window", "-n"],
            Self::Attach => &["attach", "terminal", "herdr"],
            Self::AttachSession => &["attach", "session", "herdr"],
            Self::Worktrees => &["worktree", "git", "branch"],
            Self::Resnapshot => &["snapshot", "resync", "refresh"],
            Self::RefreshAgents => &["agents", "list", "refresh"],
            Self::Notify => &["notify", "toast", "notification"],
        }
    }

    pub fn all() -> &'static [PaletteAction] {
        &[
            Self::Focus,
            Self::Peek,
            Self::Send,
            Self::Start,
            Self::WaitDone,
            Self::WaitBlocked,
            Self::OpenZed,
            Self::OpenZedNew,
            Self::Attach,
            Self::AttachSession,
            Self::Worktrees,
            Self::Resnapshot,
            Self::RefreshAgents,
            Self::Notify,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct Palette {
    pub query: String,
    pub selected: usize,
}

impl Palette {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            selected: 0,
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.selected = 0;
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.selected = 0;
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    pub fn filtered(&self) -> Vec<PaletteAction> {
        let q = self.query.to_lowercase();
        if q.is_empty() {
            return PaletteAction::all().to_vec();
        }
        PaletteAction::all()
            .iter()
            .copied()
            .filter(|a| {
                a.label().to_lowercase().contains(&q)
                    || a.keywords().iter().any(|k| k.contains(&q) || q.contains(k))
            })
            .collect()
    }

    pub fn move_sel(&mut self, delta: i32) {
        let n = self.filtered().len() as i32;
        if n == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as i32 + delta).rem_euclid(n) as usize;
    }

    pub fn selected_action(&self) -> Option<PaletteAction> {
        self.filtered().get(self.selected).copied()
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}
