//! Async intent worker — Herdr RPC + editor/attach side effects.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acex_editor::{EditorBridge, OpenMode, ZedBridge};
use acex_model::{Intent, Store, ZedOpenMode};
use herdr_client::{
    connect_with_optional_spawn, extract_agent_rows, extract_read_text, resync_with_backoff,
    SocketTarget,
};
use tracing::{info, warn};

use crate::sync_util::lock_store;

pub async fn run_intent_worker(
    target: SocketTarget,
    spawn: bool,
    editor_bin: String,
    peek_lines: u32,
    store: Arc<Mutex<Store>>,
    rx: Receiver<Intent>,
) {
    let editor = ZedBridge::new(editor_bin);

    loop {
        // try_recv + sleep keeps one runtime simple for MVP.
        let intent = match rx.try_recv() {
            Ok(i) => i,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(40)).await;
                continue;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        };

        if let Err(e) = handle_intent(&target, spawn, peek_lines, &editor, &store, intent).await {
            warn!(error = %e, "intent failed");
            let mut s = lock_store(store.as_ref());
            s.set_error(format!("action failed: {e}"));
        }
    }
}

async fn handle_intent(
    target: &SocketTarget,
    spawn: bool,
    peek_lines: u32,
    editor: &ZedBridge,
    store: &Arc<Mutex<Store>>,
    intent: Intent,
) -> anyhow::Result<()> {
    match intent {
        Intent::FocusSelected | Intent::FocusTarget(_) => {
            let t = match &intent {
                Intent::FocusTarget(t) => t.clone(),
                _ => selected_target(store)?,
            };
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client.agent_focus(&t).await?;
            info!(%t, ?r, "agent.focus");
            let mut s = lock_store(store.as_ref());
            s.set_toast(format!("focused {t}"));
            s.last_error = None;
        }
        Intent::PeekSelected | Intent::PeekTarget(_) => {
            let t = match &intent {
                Intent::PeekTarget(t) => t.clone(),
                _ => selected_target(store)?,
            };
            {
                let mut s = lock_store(store.as_ref());
                s.peek_busy = true;
            }
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let result = match client.agent_read(&t, "recent", peek_lines, true).await {
                Ok(v) => v,
                Err(e) => {
                    warn!(error = %e, "agent.read failed; trying pane.read");
                    client.pane_read(&t, "recent", peek_lines, true).await?
                }
            };
            let text = extract_read_text(&result);
            let mut s = lock_store(store.as_ref());
            s.set_peek(t, &text);
            s.set_toast("peek updated");
            s.last_error = None;
        }
        Intent::SendSelected { text } => {
            let t = selected_target(store)?;
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client.agent_send(&t, &text).await?;
            info!(%t, ?r, "agent.send");
            if let Ok(v) = client.agent_read(&t, "recent", peek_lines, true).await {
                let mut s = lock_store(store.as_ref());
                s.set_peek(&t, &extract_read_text(&v));
            }
            let mut s = lock_store(store.as_ref());
            s.set_toast(format!("sent to {t}"));
            s.last_error = None;
        }
        Intent::StartAgent { name, argv, cwd } => {
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let cwd_ref = cwd.as_deref();
            let r = client.agent_start(&name, &argv, cwd_ref, true).await?;
            info!(%name, ?r, "agent.start");
            if let Ok(list) = client.agent_list().await {
                let rows = extract_agent_rows(&list);
                let mut s = lock_store(store.as_ref());
                s.merge_agent_values(&rows);
            }
            let mut s = lock_store(store.as_ref());
            s.set_toast(format!("started {name}"));
            s.last_error = None;
        }
        Intent::WaitSelected { status } => {
            let t = selected_target(store)?;
            let mut s = lock_store(store.as_ref());
            s.arm_wait(t, status);
        }
        Intent::OpenZed { path, mode } => {
            let path = resolve_open_path(store, path)?;
            let om = match mode {
                ZedOpenMode::Default => OpenMode::Default,
                ZedOpenMode::NewWindow => OpenMode::NewWindow,
                ZedOpenMode::AddToWindow => OpenMode::AddToWindow,
            };
            editor.open_path(&path, om)?;
            let mut s = lock_store(store.as_ref());
            s.set_toast(format!("zed {}", path.display()));
            s.last_error = None;
        }
        Intent::DiffZed { old, new } => {
            editor.diff(std::path::Path::new(&old), std::path::Path::new(&new))?;
            let mut s = lock_store(store.as_ref());
            s.set_toast("zed --diff");
        }
        Intent::AttachSelected => {
            let t = selected_target(store)?;
            spawn_herdr_attach(Some(&t))?;
            let mut s = lock_store(store.as_ref());
            s.set_toast(format!("attach {t}"));
        }
        Intent::AttachSession => {
            spawn_herdr_attach(None)?;
            let mut s = lock_store(store.as_ref());
            s.set_toast("herdr attach session");
        }
        Intent::WorktreeList => {
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client.worktree_list(None, None).await?;
            let lines = format_worktrees(&r);
            let n = lines.len();
            let mut s = lock_store(store.as_ref());
            s.worktrees = lines;
            s.set_toast(format!("worktrees {n}"));
            s.last_error = None;
        }
        Intent::Resnapshot => {
            let (_pong, snap) = resync_with_backoff(target, spawn, 12).await?;
            let mut s = lock_store(store.as_ref());
            s.apply_resnapshot(snap);
            s.set_toast("resnapshot ok");
        }
        Intent::Notify { title, body } => {
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client.notification_show(&title, body.as_deref()).await?;
            info!(?r, "notification.show");
            let mut s = lock_store(store.as_ref());
            s.set_toast(format!("notify: {title}"));
        }
        Intent::RefreshAgents => {
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            match client.agent_list().await {
                Ok(list) => {
                    let rows = extract_agent_rows(&list);
                    let mut s = lock_store(store.as_ref());
                    if rows.is_empty() {
                        let snap = s.snapshot.clone();
                        s.apply_snapshot(snap);
                        s.set_toast("agent.list empty · panes");
                    } else {
                        s.agents.clear();
                        s.merge_agent_values(&rows);
                        let n = s.agents.len();
                        s.set_toast(format!("agents {n}"));
                    }
                    s.last_error = None;
                }
                Err(e) => {
                    let mut s = lock_store(store.as_ref());
                    let snap = s.snapshot.clone();
                    s.apply_snapshot(snap);
                    s.set_error(format!("agent.list failed ({e}); rebuilt from panes"));
                }
            }
        }
    }
    Ok(())
}

fn selected_target(store: &Arc<Mutex<Store>>) -> anyhow::Result<String> {
    let s = lock_store(store.as_ref());
    s.selected_target()
        .ok_or_else(|| anyhow::anyhow!("no agent selected"))
}

fn resolve_open_path(store: &Arc<Mutex<Store>>, path: Option<String>) -> anyhow::Result<PathBuf> {
    if let Some(p) = path {
        return Ok(PathBuf::from(p));
    }
    let s = lock_store(store.as_ref());
    if let Some(a) = s.selected_agent() {
        if let Some(cwd) = &a.cwd {
            return Ok(PathBuf::from(cwd));
        }
    }
    for ws in &s.snapshot.workspaces {
        if let Some(cwd) = ws.get("cwd").and_then(|v| v.as_str()) {
            return Ok(PathBuf::from(cwd));
        }
    }
    Ok(std::env::current_dir()?)
}

fn spawn_herdr_attach(target: Option<&str>) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("herdr");
    if let Some(t) = target {
        cmd.args(["agent", "attach", t]);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("herdr not found on PATH")
        }
        Err(e) => Err(e.into()),
    }
}

fn format_worktrees(result: &serde_json::Value) -> Vec<String> {
    let arr = result
        .get("worktrees")
        .and_then(|v| v.as_array())
        .cloned()
        .or_else(|| result.as_array().cloned())
        .unwrap_or_default();
    arr.iter()
        .map(|w| {
            let path = w
                .get("path")
                .or_else(|| w.get("cwd"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let branch = w.get("branch").and_then(|v| v.as_str()).unwrap_or("-");
            format!("{branch}  {path}")
        })
        .collect()
}
