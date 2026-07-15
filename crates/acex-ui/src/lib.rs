//! ratatui shell for acex. Only this crate draws.

mod palette;

use acex_model::{
    AgentState, AttachTarget, ConnState, Intent, StartPreset, Store, WaitBadge, WorktreeCreateSpec,
    WorktreeOpenSpec, WorktreeRemoveSpec,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use palette::{Palette, PaletteAction};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;
use std::io::{self, stdout};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
enum Mode {
    #[default]
    Board,
    Palette,
    /// Freeform text input; on submit maps to intent builder.
    Input {
        title: String,
        buffer: String,
        kind: InputKind,
    },
}

#[derive(Debug, Clone)]
enum InputKind {
    Send,
    StartAgent,
    FilterQuery,
    Notify,
    WorktreeCreate,
    WorktreeOpen,
    WorktreeRemove,
}

pub struct App {
    pub store: Arc<Mutex<Store>>,
    pub should_quit: bool,
    mode: Mode,
    palette: Palette,
    intent_tx: Option<Sender<Intent>>,
    status_flash: Option<String>,
    start_presets: Vec<StartPreset>,
}

impl App {
    pub fn with_shared(store: Arc<Mutex<Store>>, intent_tx: Sender<Intent>) -> Self {
        Self::with_shared_and_presets(store, intent_tx, Vec::new())
    }

    pub fn with_shared_and_presets(
        store: Arc<Mutex<Store>>,
        intent_tx: Sender<Intent>,
        start_presets: Vec<StartPreset>,
    ) -> Self {
        Self {
            store,
            should_quit: false,
            mode: Mode::Board,
            palette: Palette::new(),
            intent_tx: Some(intent_tx),
            status_flash: None,
            start_presets,
        }
    }

    pub fn store_handle(&self) -> Arc<Mutex<Store>> {
        Arc::clone(&self.store)
    }

    fn send_intent(&mut self, intent: Intent) {
        if let Some(tx) = &self.intent_tx {
            if tx.send(intent).is_err() {
                self.status_flash = Some("intent channel closed".into());
            }
        }
    }
}

pub fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    {
        let mut s = app.store.lock().unwrap_or_else(|e| e.into_inner());
        s.ensure_selection();
    }

    let result = loop {
        {
            let mut store = app.store.lock().unwrap_or_else(|e| e.into_inner());
            store.expire_waits(now_ms());
            terminal.draw(|f| draw(f, &store, &app))?;
        }

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(&mut app, key.code, key.modifiers);
                }
            }
        }

        if app.should_quit {
            break Ok(());
        }
    };

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    result
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match &app.mode {
        Mode::Palette => match code {
            KeyCode::Esc => app.mode = Mode::Board,
            KeyCode::Char('q') if mods.is_empty() => app.mode = Mode::Board,
            KeyCode::Up | KeyCode::Char('k') => app.palette.move_sel(-1),
            KeyCode::Down | KeyCode::Char('j') => app.palette.move_sel(1),
            KeyCode::Backspace => app.palette.backspace(),
            KeyCode::Enter => {
                if let Some(action) = app.palette.selected_action() {
                    apply_palette_action(app, action);
                }
            }
            KeyCode::Char(c) if !mods.contains(KeyModifiers::CONTROL) => {
                app.palette.push_char(c);
            }
            _ => {}
        },
        Mode::Input { .. } => match code {
            KeyCode::Esc => app.mode = Mode::Board,
            KeyCode::Enter => submit_input(app),
            KeyCode::Backspace => {
                if let Mode::Input { buffer, .. } = &mut app.mode {
                    buffer.pop();
                }
            }
            KeyCode::Char(c) if !mods.contains(KeyModifiers::CONTROL) => {
                if let Mode::Input { buffer, .. } = &mut app.mode {
                    buffer.push(c);
                }
            }
            _ => {}
        },
        Mode::Board => {
            if mods.contains(KeyModifiers::CONTROL)
                && matches!(code, KeyCode::Char('k') | KeyCode::Char('K'))
            {
                app.palette.reset();
                app.mode = Mode::Palette;
                return;
            }
            match code {
                KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                KeyCode::Up | KeyCode::Char('k') => {
                    let mut s = app.store.lock().unwrap_or_else(|e| e.into_inner());
                    s.select_delta(-1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let mut s = app.store.lock().unwrap_or_else(|e| e.into_inner());
                    s.select_delta(1);
                }
                KeyCode::Char('f') | KeyCode::Enter => app.send_intent(Intent::FocusSelected),
                KeyCode::Char('r') => app.send_intent(Intent::PeekSelected),
                KeyCode::Char('s') => {
                    app.mode = Mode::Input {
                        title: "send to selected agent".into(),
                        buffer: String::new(),
                        kind: InputKind::Send,
                    };
                }
                KeyCode::Char('w') => {
                    app.send_intent(Intent::WaitSelected {
                        status: AgentState::Done,
                    });
                }
                KeyCode::Char('b') => {
                    app.send_intent(Intent::WaitSelected {
                        status: AgentState::Blocked,
                    });
                }
                KeyCode::Char('a') => app.send_intent(Intent::Attach {
                    target: AttachTarget::SelectedAgent,
                }),
                KeyCode::Char('z') => app.send_intent(Intent::OpenZed {
                    path: None,
                    mode: acex_model::ZedOpenMode::Default,
                }),
                KeyCode::Char('t') => app.send_intent(Intent::WorktreeList),
                KeyCode::Char('R') => app.send_intent(Intent::Resnapshot),
                KeyCode::Char('A') => app.send_intent(Intent::RefreshAgents),
                KeyCode::Char('/') => {
                    app.mode = Mode::Input {
                        title: "filter query".into(),
                        buffer: String::new(),
                        kind: InputKind::FilterQuery,
                    };
                }
                KeyCode::Char('1') => set_filter(app, None),
                KeyCode::Char('2') => set_filter(app, Some(AgentState::Idle)),
                KeyCode::Char('3') => set_filter(app, Some(AgentState::Working)),
                KeyCode::Char('4') => set_filter(app, Some(AgentState::Blocked)),
                KeyCode::Char('5') => set_filter(app, Some(AgentState::Done)),
                KeyCode::Char('0') => {
                    let mut s = app.store.lock().unwrap_or_else(|e| e.into_inner());
                    s.cycle_filter_state();
                }
                KeyCode::Char('p') | KeyCode::Char(' ') => {
                    app.palette.reset();
                    app.mode = Mode::Palette;
                }
                _ => {}
            }
        }
    }
}

fn set_filter(app: &mut App, state: Option<AgentState>) {
    let mut s = app.store.lock().unwrap_or_else(|e| e.into_inner());
    s.set_filter_state(state);
}

fn submit_input(app: &mut App) {
    let mode = std::mem::replace(&mut app.mode, Mode::Board);
    if let Mode::Input { buffer, kind, .. } = mode {
        match kind {
            InputKind::Send => {
                if !buffer.is_empty() {
                    app.send_intent(Intent::SendSelected { text: buffer });
                }
            }
            InputKind::StartAgent => {
                let (name, argv, cwd) = parse_start(&buffer, &app.start_presets);
                if !argv.is_empty() {
                    app.send_intent(Intent::StartAgent { name, argv, cwd });
                }
            }
            InputKind::FilterQuery => {
                let mut s = app.store.lock().unwrap_or_else(|e| e.into_inner());
                s.filter.query = buffer;
                s.ensure_selection();
            }
            InputKind::Notify => {
                if !buffer.is_empty() {
                    app.send_intent(Intent::Notify {
                        title: buffer,
                        body: None,
                    });
                }
            }
            InputKind::WorktreeCreate => match parse_worktree_create(&buffer) {
                Some(spec) => app.send_intent(Intent::WorktreeCreate(spec)),
                None => app.status_flash = Some("worktree create needs branch= or path=".into()),
            },
            InputKind::WorktreeOpen => match parse_worktree_open(&buffer) {
                Some(spec) => app.send_intent(Intent::WorktreeOpen(spec)),
                None => {
                    app.status_flash =
                        Some("worktree open needs path=, branch=, or workspace=".into())
                }
            },
            InputKind::WorktreeRemove => match parse_worktree_remove(&buffer) {
                Some(spec) => app.send_intent(Intent::WorktreeRemove(spec)),
                None => {
                    app.status_flash =
                        Some("worktree remove needs workspace id; add --force explicitly".into())
                }
            },
        }
    }
}

fn parse_start(raw: &str, presets: &[StartPreset]) -> (String, Vec<String>, Option<String>) {
    let raw = raw.trim();
    if let Some(preset) = find_start_preset(raw, presets) {
        return (preset.name.clone(), preset.argv.clone(), preset.cwd.clone());
    }
    if let Some((name, rest)) = raw.split_once('|') {
        let argv = shellish_split(rest.trim());
        (name.trim().to_string(), argv, None)
    } else {
        let argv = shellish_split(raw);
        let name = argv.first().cloned().unwrap_or_else(|| "agent".into());
        (name, argv, None)
    }
}

fn find_start_preset<'a>(raw: &str, presets: &'a [StartPreset]) -> Option<&'a StartPreset> {
    let selector = raw.strip_prefix('@').unwrap_or(raw);
    presets
        .iter()
        .find(|preset| preset.id == selector || preset.name == selector)
}

fn start_input_title(presets: &[StartPreset]) -> String {
    if presets.is_empty() {
        return "start agent · name|cmd args  OR  cmd args".into();
    }
    let ids = presets
        .iter()
        .map(|preset| preset.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("start agent · @{ids} OR name|cmd args OR cmd args")
}

fn shellish_split(s: &str) -> Vec<String> {
    s.split_whitespace().map(|p| p.to_string()).collect()
}

fn parse_worktree_create(raw: &str) -> Option<WorktreeCreateSpec> {
    let mut spec = WorktreeCreateSpec {
        branch: None,
        path: None,
        base: None,
        label: None,
        cwd: None,
        workspace_id: None,
        focus: true,
    };

    for token in shellish_split(raw) {
        if token == "--no-focus" {
            spec.focus = false;
            continue;
        }
        if token.starts_with('-') {
            return None;
        }
        if let Some((key, value)) = token.split_once('=') {
            let value = non_empty_value(value)?;
            match key {
                "branch" => spec.branch = Some(value),
                "path" => spec.path = Some(value),
                "base" => spec.base = Some(value),
                "label" => spec.label = Some(value),
                "cwd" => spec.cwd = Some(value),
                "workspace" | "workspace_id" => spec.workspace_id = Some(value),
                _ => return None,
            }
        } else if spec.branch.is_none() {
            spec.branch = Some(token);
        } else if spec.path.is_none() {
            spec.path = Some(token);
        } else if spec.base.is_none() {
            spec.base = Some(token);
        } else {
            return None;
        }
    }

    (spec.branch.is_some() || spec.path.is_some()).then_some(spec)
}

fn parse_worktree_open(raw: &str) -> Option<WorktreeOpenSpec> {
    let mut spec = WorktreeOpenSpec {
        branch: None,
        path: None,
        label: None,
        cwd: None,
        workspace_id: None,
        focus: true,
    };

    for token in shellish_split(raw) {
        if token == "--no-focus" {
            spec.focus = false;
            continue;
        }
        if token.starts_with('-') {
            return None;
        }
        if let Some((key, value)) = token.split_once('=') {
            let value = non_empty_value(value)?;
            match key {
                "branch" => spec.branch = Some(value),
                "path" => spec.path = Some(value),
                "label" => spec.label = Some(value),
                "cwd" => spec.cwd = Some(value),
                "workspace" | "workspace_id" => spec.workspace_id = Some(value),
                _ => return None,
            }
        } else if spec.path.is_none() {
            spec.path = Some(token);
        } else if spec.branch.is_none() {
            spec.branch = Some(token);
        } else {
            return None;
        }
    }

    (spec.path.is_some() || spec.branch.is_some() || spec.workspace_id.is_some()).then_some(spec)
}

fn parse_worktree_remove(raw: &str) -> Option<WorktreeRemoveSpec> {
    let mut workspace_id = None;
    let mut force = false;

    for token in shellish_split(raw) {
        if token == "--force" {
            force = true;
            continue;
        }
        if token.starts_with('-') {
            return None;
        }
        if let Some((key, value)) = token.split_once('=') {
            let value = non_empty_value(value)?;
            match key {
                "workspace" | "workspace_id" | "id" => workspace_id = Some(value),
                _ => return None,
            }
        } else if workspace_id.is_none() {
            workspace_id = Some(token);
        } else {
            return None;
        }
    }

    workspace_id.map(|workspace_id| WorktreeRemoveSpec {
        workspace_id,
        force,
    })
}

fn non_empty_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn apply_palette_action(app: &mut App, action: PaletteAction) {
    app.mode = Mode::Board;
    match action {
        PaletteAction::Focus => app.send_intent(Intent::FocusSelected),
        PaletteAction::Peek => app.send_intent(Intent::PeekSelected),
        PaletteAction::Send => {
            app.mode = Mode::Input {
                title: "send to selected agent".into(),
                buffer: String::new(),
                kind: InputKind::Send,
            };
        }
        PaletteAction::Start => {
            app.mode = Mode::Input {
                title: start_input_title(&app.start_presets),
                buffer: String::new(),
                kind: InputKind::StartAgent,
            };
        }
        PaletteAction::WaitDone => {
            app.send_intent(Intent::WaitSelected {
                status: AgentState::Done,
            });
        }
        PaletteAction::WaitBlocked => {
            app.send_intent(Intent::WaitSelected {
                status: AgentState::Blocked,
            });
        }
        PaletteAction::OpenZed => {
            app.send_intent(Intent::OpenZed {
                path: None,
                mode: acex_model::ZedOpenMode::Default,
            });
        }
        PaletteAction::OpenZedNew => {
            app.send_intent(Intent::OpenZed {
                path: None,
                mode: acex_model::ZedOpenMode::NewWindow,
            });
        }
        PaletteAction::Attach => app.send_intent(Intent::Attach {
            target: AttachTarget::SelectedAgent,
        }),
        PaletteAction::AttachSession => app.send_intent(Intent::Attach {
            target: AttachTarget::Session,
        }),
        PaletteAction::WorktreeList => app.send_intent(Intent::WorktreeList),
        PaletteAction::WorktreeCreate => {
            app.mode = Mode::Input {
                title: "worktree create · branch=<name> [path=… base=…] [--no-focus]".into(),
                buffer: String::new(),
                kind: InputKind::WorktreeCreate,
            };
        }
        PaletteAction::WorktreeOpen => {
            app.mode = Mode::Input {
                title: "worktree open · path=… or branch=… or workspace=… [--no-focus]".into(),
                buffer: String::new(),
                kind: InputKind::WorktreeOpen,
            };
        }
        PaletteAction::WorktreeRemove => {
            app.mode = Mode::Input {
                title: "worktree remove · workspace id [--force] (force must be explicit)".into(),
                buffer: String::new(),
                kind: InputKind::WorktreeRemove,
            };
        }
        PaletteAction::Resnapshot => app.send_intent(Intent::Resnapshot),
        PaletteAction::RefreshAgents => app.send_intent(Intent::RefreshAgents),
        PaletteAction::Notify => {
            app.mode = Mode::Input {
                title: "notification title".into(),
                buffer: String::new(),
                kind: InputKind::Notify,
            };
        }
    }
}

fn draw(f: &mut ratatui::Frame, store: &Store, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(f, chunks[0], store);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(chunks[1]);

    draw_board(f, body[0], store);
    draw_detail(f, body[1], store);
    draw_footer(f, chunks[2], store, app);

    match &app.mode {
        Mode::Palette => draw_palette_modal(f, area, app),
        Mode::Input { title, buffer, .. } => draw_input_modal(f, area, title, buffer),
        Mode::Board => {}
    }
}

fn draw_header(f: &mut ratatui::Frame, area: Rect, store: &Store) {
    let status = match store.conn {
        ConnState::Offline => "offline",
        ConnState::Connecting => "connecting…",
        ConnState::Live if store.subscribed => "live●",
        ConnState::Live => "live",
        ConnState::Reconnecting => "reconnecting…",
    };
    let title = Line::from(vec![
        Span::styled(" ACEX ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("· "),
        Span::styled(status, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!(
            " · ev {} · gen {} · re {}",
            store.event_count, store.snapshot_gen, store.reconnect_count
        )),
    ]);
    let p = Paragraph::new(title).block(Block::default().borders(Borders::ALL).title("acex"));
    f.render_widget(p, area);
}

fn draw_board(f: &mut ratatui::Frame, area: Rect, store: &Store) {
    let counts = store.state_counts();
    let filter_label = match store.filter.state {
        None => "all".to_string(),
        Some(s) => s.as_str().to_string(),
    };
    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(format!(
        "filter:{filter_label} q={:?}  [i{} w{} b{} d{} u{}]",
        store.filter.query, counts[0].1, counts[1].1, counts[2].1, counts[3].1, counts[4].1,
    )));

    let rows = store.filtered_agents();
    if rows.is_empty() {
        items.push(ListItem::new("  (empty board) · Ctrl+K start"));
    } else {
        for (idx, a) in rows {
            let sel = store.selected == Some(idx);
            let mark = if sel { "▶" } else { " " };
            let name = a.name.as_deref().unwrap_or(a.id.as_str());
            let target = a.pane_id.as_deref().unwrap_or(a.id.as_str());
            let wait = store
                .waits
                .iter()
                .find(|w| w.target == target)
                .map(wait_indicator)
                .unwrap_or_default();
            let line = format!(
                "{mark} [{:>8}] {} ({}){wait}",
                a.state.as_str(),
                name,
                target
            );
            let item = if sel {
                ListItem::new(line).style(Style::default().add_modifier(Modifier::BOLD))
            } else {
                ListItem::new(line)
            };
            items.push(item);
        }
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("agents · j/k · 1-5 filter · / search"),
    );
    f.render_widget(list, area);
}

fn draw_detail(f: &mut ratatui::Frame, area: Rect, store: &Store) {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(a) = store.selected_agent() {
        lines.push(Line::from(format!(
            "selected · {} · {}",
            a.pane_id.as_deref().unwrap_or(&a.id),
            a.state.as_str()
        )));
        if let Some(cwd) = &a.cwd {
            lines.push(Line::from(format!("cwd {cwd}")));
        }
    } else {
        lines.push(Line::from("no selection"));
    }

    if let Some(toast) = &store.toast {
        lines.push(Line::from(format!("toast: {toast}")));
    }
    if let Some(note) = &store.status_note {
        lines.push(Line::from(note.as_str()));
    }
    if let Some(err) = &store.last_error {
        lines.push(Line::from(format!("! {err}")));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "peek {} · {}",
        store.peek_target.as_deref().unwrap_or("-"),
        if store.peek_busy { "…" } else { "ready" }
    )));
    if store.peek_lines.is_empty() {
        lines.push(Line::from("  (r refresh peek)"));
    } else {
        let take = store.peek_lines.len().saturating_sub(40);
        for l in store.peek_lines.iter().skip(take) {
            lines.push(Line::from(format!("  {l}")));
        }
    }

    if !store.worktrees.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from("worktrees:"));
        for w in store.worktrees.iter().take(8) {
            lines.push(Line::from(format!("  · {w}")));
        }
    }

    if !store.waits.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from("waits:"));
        for w in &store.waits {
            lines.push(Line::from(format!("  · {}", w.message)));
        }
    }

    if !store.recent_events.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from("events:"));
        for e in store.recent_events.iter().rev().take(5) {
            lines.push(Line::from(format!("  · {e}")));
        }
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("detail · peek"),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_footer(f: &mut ratatui::Frame, area: Rect, store: &Store, app: &App) {
    let flash = app
        .status_flash
        .as_deref()
        .or(store.toast.as_deref())
        .unwrap_or("");
    let help = format!(
        " Ctrl+K palette · f focus · r peek · s send · w/b wait · a attach · z zed · t worktrees · q quit  {flash}"
    );
    let p = Paragraph::new(help).block(Block::default().borders(Borders::ALL).title("keys"));
    f.render_widget(p, area);
}
fn wait_indicator(w: &WaitBadge) -> String {
    if w.armed {
        format!(" ⏳{}", w.want.as_str())
    } else if w.resolved {
        format!(" ✓{}", w.want.as_str())
    } else if w.expired {
        format!(" ⚠{} expired", w.want.as_str())
    } else {
        String::new()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn draw_palette_modal(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let popup = centered_rect(60, 60, area);
    f.render_widget(Clear, popup);
    let items: Vec<ListItem> = app
        .palette
        .filtered()
        .into_iter()
        .enumerate()
        .map(|(i, a)| {
            let mark = if i == app.palette.selected {
                "▶ "
            } else {
                "  "
            };
            ListItem::new(format!("{mark}{}", a.label()))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("palette · {}", app.palette.query)),
    );
    f.render_widget(list, popup);
}

fn draw_input_modal(f: &mut ratatui::Frame, area: Rect, title: &str, buffer: &str) {
    let popup = centered_rect(70, 20, area);
    f.render_widget(Clear, popup);
    let p = Paragraph::new(format!("{buffer}▌")).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_bottom(" enter submit · esc cancel "),
    );
    f.render_widget(p, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_parser_resolves_configured_preset() {
        let presets = vec![StartPreset {
            id: "review".into(),
            name: "reviewer".into(),
            argv: vec!["omp".into(), "--agent".into(), "reviewer".into()],
            cwd: Some("crates".into()),
        }];

        let (name, argv, cwd) = parse_start("@review", &presets);

        assert_eq!(name, "reviewer");
        assert_eq!(argv, ["omp", "--agent", "reviewer"]);
        assert_eq!(cwd.as_deref(), Some("crates"));
    }

    #[test]
    fn start_parser_preserves_freeform_fallback() {
        let (name, argv, cwd) = parse_start("worker | cargo test -p acex", &[]);

        assert_eq!(name, "worker");
        assert_eq!(argv, ["cargo", "test", "-p", "acex"]);
        assert_eq!(cwd, None);
    }

    #[test]
    fn worktree_create_parser_accepts_explicit_fields() {
        let spec = parse_worktree_create("branch=feature path=../feature base=main --no-focus")
            .expect("create spec");

        assert_eq!(spec.branch.as_deref(), Some("feature"));
        assert_eq!(spec.path.as_deref(), Some("../feature"));
        assert_eq!(spec.base.as_deref(), Some("main"));
        assert!(!spec.focus);
    }

    #[test]
    fn worktree_open_parser_requires_handoff_target() {
        assert!(parse_worktree_open("").is_none());

        let spec = parse_worktree_open("workspace=ws-1").expect("open spec");
        assert_eq!(spec.workspace_id.as_deref(), Some("ws-1"));
        assert!(spec.focus);
    }

    #[test]
    fn worktree_remove_parser_requires_explicit_force_flag() {
        let safe = parse_worktree_remove("ws-1").expect("safe remove");
        assert_eq!(safe.workspace_id, "ws-1");
        assert!(!safe.force);

        let forced = parse_worktree_remove("workspace=ws-1 --force").expect("forced remove");
        assert_eq!(forced.workspace_id, "ws-1");
        assert!(forced.force);
    }

    #[test]
    fn wait_indicator_shows_expired_state() {
        let wait = WaitBadge {
            target: "w1:p1".into(),
            want: AgentState::Done,
            armed: false,
            resolved: false,
            expired: true,
            expires_at_ms: Some(30_000),
            message: "wait expired".into(),
        };

        assert_eq!(wait_indicator(&wait), " ⚠done expired");
    }
}
