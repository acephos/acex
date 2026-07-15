//! Async intent worker — Herdr RPC + editor/attach side effects.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use acex_editor::{EditorBridge, OpenMode, ZedBridge};
use acex_model::{
    AttachTarget, Intent, LayoutPreset, Store, WorkspaceFocusSpec, WorktreeCreateSpec,
    WorktreeOpenSpec, ZedOpenMode, DEFAULT_WAIT_TIMEOUT_MS,
};
use herdr_client::{
    connect_with_optional_spawn, extract_agent_rows, extract_read_text,
    resolve::session_socket_path, resync_with_backoff, LayoutApplyRequest, SocketTarget,
    WorktreeCreateRequest, WorktreeOpenRequest,
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
    editor: &dyn EditorBridge,
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
            let result = match client.agent_read(&t, "recent", peek_lines, false).await {
                Ok(v) => v,
                Err(e) => {
                    warn!(error = %e, "agent.read failed; trying pane.read");
                    client.pane_read(&t, "recent", peek_lines, false).await?
                }
            };
            let text = extract_read_text(&result);
            let mut s = lock_store(store.as_ref());
            s.set_peek(t, &text);
            s.set_toast("peek updated");
            s.last_error = None;
        }
        Intent::RunPaneSelected { command } => {
            let pane_id = selected_target(store)?;
            let plan = build_herdr_pane_run_plan(target, &pane_id, &command)?;
            run_herdr_command(&plan).await?;
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let result = client
                .pane_read(&pane_id, "recent", peek_lines, false)
                .await?;
            let text = extract_read_text(&result);
            let mut s = lock_store(store.as_ref());
            s.set_peek(pane_id.clone(), &text);
            s.set_toast(format!("pane run {pane_id}"));
            s.last_error = None;
        }
        Intent::SendSelected { text } => {
            let t = selected_target(store)?;
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client.agent_send(&t, &text).await?;
            info!(%t, ?r, "agent.send");
            if let Ok(v) = client.agent_read(&t, "recent", peek_lines, false).await {
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
            s.arm_wait_at(t, status, now_ms(), DEFAULT_WAIT_TIMEOUT_MS);
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
            let (old, new) = resolve_diff_paths(&old, &new)?;
            editor.diff(&old, &new)?;
            let mut s = lock_store(store.as_ref());
            s.set_toast("zed --diff");
            s.last_error = None;
        }
        Intent::ApplyLayout(preset) => {
            let label = preset
                .tab_label
                .as_deref()
                .unwrap_or(&preset.name)
                .to_string();
            let request = layout_apply_request(&preset)?;
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client.layout_apply(request).await?;
            info!(preset = %preset.id, tab = %label, ?r, "layout.apply");
            let mut s = lock_store(store.as_ref());
            s.set_toast(format!("layout {label} · new tab · no live PTY preserve"));
            s.last_error = None;
        }
        Intent::Attach { target: attach } => {
            let attach = match attach {
                AttachTarget::SelectedAgent => AttachTarget::Agent(selected_target(store)?),
                other => other,
            };
            let plan = build_herdr_attach_plan(target, &attach)?;
            spawn_herdr_attach(&plan)?;
            let mut s = lock_store(store.as_ref());
            s.set_toast(attach_toast(&attach));
            s.last_error = None;
        }
        Intent::WorkspaceList => {
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let result = client.workspace_list().await?;
            let rows = extract_workspace_values(&result);
            let n = rows.len();
            let mut s = lock_store(store.as_ref());
            s.apply_workspace_values(rows);
            s.set_toast(format!("workspaces {n}"));
            s.last_error = None;
        }
        Intent::WorkspaceFocus(spec) => {
            let workspace_id = workspace_focus_id(&spec)?;
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client.workspace_focus(&workspace_id).await?;
            info!(%workspace_id, ?r, "workspace.focus");
            let list = client.workspace_list().await.ok();
            let mut s = lock_store(store.as_ref());
            if let Some(list) = list {
                s.apply_workspace_values(extract_workspace_values(&list));
            }
            s.set_workspace_filter(Some(workspace_id.clone()));
            s.set_toast(format!("workspace focused {workspace_id}"));
            s.last_error = None;
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
        Intent::WorktreeCreate(spec) => {
            let label = spec
                .branch
                .as_deref()
                .or(spec.path.as_deref())
                .unwrap_or("worktree")
                .to_string();
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client
                .worktree_create(worktree_create_request(&spec))
                .await?;
            info!(%label, ?r, "worktree.create");
            let list = client.worktree_list(None, None).await?;
            let lines = format_worktrees(&list);
            let n = lines.len();
            let mut s = lock_store(store.as_ref());
            s.worktrees = lines;
            s.set_toast(format!("worktree created {label} · {n} listed"));
            s.last_error = None;
        }
        Intent::WorktreeOpen(spec) => {
            let label = spec
                .workspace_id
                .as_deref()
                .or(spec.path.as_deref())
                .or(spec.branch.as_deref())
                .unwrap_or("worktree")
                .to_string();
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client.worktree_open(worktree_open_request(&spec)).await?;
            info!(%label, ?r, "worktree.open");
            let list = client.worktree_list(None, None).await?;
            let lines = format_worktrees(&list);
            let n = lines.len();
            let mut s = lock_store(store.as_ref());
            s.worktrees = lines;
            s.set_toast(format!(
                "worktree opened {label} · Herdr handoff · {n} listed"
            ));
            s.last_error = None;
        }
        Intent::WorktreeRemove(spec) => {
            let mut client = connect_with_optional_spawn(target, spawn).await?;
            let r = client
                .worktree_remove(&spec.workspace_id, spec.force)
                .await?;
            info!(workspace_id = %spec.workspace_id, force = spec.force, ?r, "worktree.remove");
            let list = client.worktree_list(None, None).await?;
            let lines = format_worktrees(&list);
            let n = lines.len();
            let mut s = lock_store(store.as_ref());
            s.worktrees = lines;
            let forced = if spec.force { " forced" } else { "" };
            s.set_toast(format!(
                "worktree removed{forced} {} · {n} listed",
                spec.workspace_id
            ));
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

fn worktree_create_request(spec: &WorktreeCreateSpec) -> WorktreeCreateRequest<'_> {
    WorktreeCreateRequest {
        branch: spec.branch.as_deref(),
        path: spec.path.as_deref(),
        base: spec.base.as_deref(),
        label: spec.label.as_deref(),
        cwd: spec.cwd.as_deref(),
        workspace_id: spec.workspace_id.as_deref(),
        focus: spec.focus,
    }
}

fn worktree_open_request(spec: &WorktreeOpenSpec) -> WorktreeOpenRequest<'_> {
    WorktreeOpenRequest {
        branch: spec.branch.as_deref(),
        path: spec.path.as_deref(),
        label: spec.label.as_deref(),
        cwd: spec.cwd.as_deref(),
        workspace_id: spec.workspace_id.as_deref(),
        focus: spec.focus,
    }
}

fn layout_apply_request(preset: &LayoutPreset) -> anyhow::Result<LayoutApplyRequest<'_>> {
    let root = serde_json::to_value(&preset.root)?;
    Ok(LayoutApplyRequest {
        root,
        tab_label: preset.tab_label.as_deref().or(Some(preset.name.as_str())),
        workspace_id: preset.workspace_id.as_deref(),
        focus: preset.focus,
    })
}
fn workspace_focus_id(spec: &WorkspaceFocusSpec) -> anyhow::Result<String> {
    non_blank(&spec.workspace_id, "workspace focus target")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HerdrCommandPlan {
    program: &'static str,
    args: Vec<String>,
    env: Vec<(&'static str, OsString)>,
}

fn build_herdr_attach_plan(
    socket_target: &SocketTarget,
    attach_target: &AttachTarget,
) -> anyhow::Result<HerdrCommandPlan> {
    let mut plan = HerdrCommandPlan {
        program: "herdr",
        args: Vec::new(),
        env: socket_env(socket_target)?,
    };

    match attach_target {
        AttachTarget::SelectedAgent => {
            anyhow::bail!("selected agent attach target must be resolved before spawning herdr")
        }
        AttachTarget::Agent(target) => {
            let target = non_blank(target, "agent attach target")?;
            plan.args.extend(["agent".into(), "attach".into(), target]);
        }
        AttachTarget::Session => {
            if let SocketTarget::Session(name) = socket_target {
                let name = non_blank(name, "session attach target")?;
                plan.args.extend(["session".into(), "attach".into(), name]);
            }
        }
    }

    Ok(plan)
}

fn socket_env(target: &SocketTarget) -> anyhow::Result<Vec<(&'static str, OsString)>> {
    let mut env = Vec::with_capacity(2);
    match target {
        SocketTarget::Path(path) => env.push(("HERDR_SOCKET_PATH", path.as_os_str().to_owned())),
        SocketTarget::Session(name) => {
            let name = non_blank(name, "session attach target")?;
            env.push((
                "HERDR_SOCKET_PATH",
                session_socket_path(&name).into_os_string(),
            ));
            env.push(("HERDR_SESSION", OsString::from(name)));
        }
        SocketTarget::Default => {
            if let Some(path) = target.path_hint() {
                env.push(("HERDR_SOCKET_PATH", path.into_os_string()));
            }
        }
    }
    Ok(env)
}

fn non_blank(value: &str, label: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{label} is blank")
    }
    Ok(trimmed.to_string())
}

fn resolve_diff_paths(old: &str, new: &str) -> anyhow::Result<(PathBuf, PathBuf)> {
    Ok((
        resolve_existing_path(old, "zed diff old path")?,
        resolve_existing_path(new, "zed diff new path")?,
    ))
}

fn resolve_existing_path(raw: &str, label: &str) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(non_blank(raw, label)?);
    if !path.try_exists()? {
        anyhow::bail!("{label} does not exist: {}", path.display());
    }
    Ok(path)
}

fn attach_toast(target: &AttachTarget) -> String {
    match target {
        AttachTarget::SelectedAgent => "attach selected".into(),
        AttachTarget::Agent(agent) => format!("attach {agent}"),
        AttachTarget::Session => "attach session".into(),
    }
}

fn build_herdr_pane_run_plan(
    socket_target: &SocketTarget,
    pane_id: &str,
    command: &str,
) -> anyhow::Result<HerdrCommandPlan> {
    let pane_id = non_blank(pane_id, "pane run target")?;
    let command = non_blank(command, "pane run command")?;
    Ok(HerdrCommandPlan {
        program: "herdr",
        args: vec!["pane".into(), "run".into(), pane_id, command],
        env: socket_env(socket_target)?,
    })
}

async fn run_herdr_command(plan: &HerdrCommandPlan) -> anyhow::Result<()> {
    let mut cmd = tokio::process::Command::new(plan.program);
    cmd.args(&plan.args);
    cmd.env_remove("HERDR_SOCKET_PATH");
    cmd.env_remove("HERDR_SESSION");
    for (key, value) in &plan.env {
        cmd.env(key, value);
    }

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().await?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    anyhow::bail!("herdr {} failed: {detail}", plan.args.join(" "))
}

fn spawn_herdr_attach(plan: &HerdrCommandPlan) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new(plan.program);
    cmd.args(&plan.args);
    cmd.env_remove("HERDR_SOCKET_PATH");
    cmd.env_remove("HERDR_SESSION");
    for (key, value) in &plan.env {
        cmd.env(key, value);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }

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

fn extract_workspace_values(result: &serde_json::Value) -> Vec<serde_json::Value> {
    result
        .get("workspaces")
        .and_then(|v| v.as_array())
        .cloned()
        .or_else(|| result.as_array().cloned())
        .unwrap_or_default()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[derive(Default)]
    struct RecordingEditor {
        diffs: Mutex<Vec<(PathBuf, PathBuf)>>,
    }

    impl EditorBridge for RecordingEditor {
        fn open_path(&self, _path: &std::path::Path, _mode: OpenMode) -> acex_editor::Result<()> {
            Ok(())
        }

        fn diff(&self, old: &std::path::Path, new: &std::path::Path) -> acex_editor::Result<()> {
            self.diffs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((old.to_path_buf(), new.to_path_buf()));
            Ok(())
        }
    }

    fn env_value<'a>(plan: &'a HerdrCommandPlan, key: &str) -> Option<&'a OsString> {
        plan.env.iter().find_map(|(k, v)| (*k == key).then_some(v))
    }

    #[test]
    fn agent_attach_uses_explicit_target_and_socket_env() {
        let socket = SocketTarget::Path(PathBuf::from("/tmp/herdr.sock"));
        let plan = build_herdr_attach_plan(&socket, &AttachTarget::Agent("agent-1".into()))
            .expect("agent attach plan");

        assert_eq!(plan.program, "herdr");
        assert_eq!(plan.args, ["agent", "attach", "agent-1"]);
        assert_eq!(
            env_value(&plan, "HERDR_SOCKET_PATH"),
            Some(&OsString::from("/tmp/herdr.sock"))
        );
        assert_eq!(env_value(&plan, "HERDR_SESSION"), None);
    }

    #[test]
    fn named_session_attach_uses_explicit_session_command_and_env() {
        let socket = SocketTarget::Session("review".into());
        let plan =
            build_herdr_attach_plan(&socket, &AttachTarget::Session).expect("session attach plan");

        assert_eq!(plan.args, ["session", "attach", "review"]);
        assert_eq!(
            env_value(&plan, "HERDR_SESSION"),
            Some(&OsString::from("review"))
        );
        assert!(env_value(&plan, "HERDR_SOCKET_PATH").is_some());
    }

    #[test]
    fn default_session_attach_falls_back_to_bare_herdr_with_socket_env() {
        let plan = build_herdr_attach_plan(&SocketTarget::Default, &AttachTarget::Session)
            .expect("default session attach plan");

        assert!(plan.args.is_empty());
        assert!(env_value(&plan, "HERDR_SOCKET_PATH").is_some());
        assert_eq!(env_value(&plan, "HERDR_SESSION"), None);
    }

    #[test]
    fn blank_agent_attach_target_is_rejected() {
        let err =
            build_herdr_attach_plan(&SocketTarget::Default, &AttachTarget::Agent("  ".into()))
                .expect_err("blank agent should fail");

        assert!(err.to_string().contains("agent attach target is blank"));
    }

    #[test]
    fn blank_session_attach_target_is_rejected() {
        let err =
            build_herdr_attach_plan(&SocketTarget::Session("  ".into()), &AttachTarget::Session)
                .expect_err("blank session should fail");

        assert!(err.to_string().contains("session attach target is blank"));
    }

    #[test]
    fn selected_agent_attach_must_be_resolved_before_command_plan() {
        let err = build_herdr_attach_plan(&SocketTarget::Default, &AttachTarget::SelectedAgent)
            .expect_err("selected target should be resolved by the worker");

        assert!(err.to_string().contains("must be resolved"));
    }

    #[test]
    fn pane_run_plan_uses_herdr_cli_and_socket_env() {
        let socket = SocketTarget::Path(PathBuf::from("/tmp/herdr.sock"));
        let plan = build_herdr_pane_run_plan(&socket, "w1:p1", "echo ok").expect("pane run plan");

        assert_eq!(plan.program, "herdr");
        assert_eq!(plan.args, ["pane", "run", "w1:p1", "echo ok"]);
        assert_eq!(
            env_value(&plan, "HERDR_SOCKET_PATH"),
            Some(&OsString::from("/tmp/herdr.sock"))
        );
    }

    #[test]
    fn pane_run_plan_rejects_blank_command() {
        let err = build_herdr_pane_run_plan(&SocketTarget::Default, "w1:p1", "  ")
            .expect_err("blank command should fail");

        assert!(err.to_string().contains("pane run command is blank"));
    }

    #[test]
    fn worktree_create_spec_maps_to_herdr_request() {
        let spec = WorktreeCreateSpec {
            branch: Some("feature".into()),
            path: Some("../feature".into()),
            base: Some("main".into()),
            label: Some("Feature".into()),
            cwd: Some("repo".into()),
            workspace_id: Some("ws-1".into()),
            focus: true,
        };

        let req = worktree_create_request(&spec);

        assert_eq!(req.branch, Some("feature"));
        assert_eq!(req.path, Some("../feature"));
        assert_eq!(req.base, Some("main"));
        assert_eq!(req.label, Some("Feature"));
        assert_eq!(req.cwd, Some("repo"));
        assert_eq!(req.workspace_id, Some("ws-1"));
        assert!(req.focus);
    }

    #[test]
    fn worktree_open_spec_maps_to_herdr_request() {
        let spec = WorktreeOpenSpec {
            branch: Some("feature".into()),
            path: Some("../feature".into()),
            label: None,
            cwd: None,
            workspace_id: Some("ws-1".into()),
            focus: false,
        };

        let req = worktree_open_request(&spec);

        assert_eq!(req.branch, Some("feature"));
        assert_eq!(req.path, Some("../feature"));
        assert_eq!(req.workspace_id, Some("ws-1"));
        assert!(!req.focus);
    }

    #[test]
    fn workspace_focus_spec_rejects_blank_target() {
        let err = workspace_focus_id(&WorkspaceFocusSpec {
            workspace_id: "  ".into(),
        })
        .expect_err("blank workspace focus target should fail");

        assert!(err.to_string().contains("workspace focus target is blank"));
    }

    #[test]
    fn extract_workspace_values_reads_list_result() {
        let rows = extract_workspace_values(&serde_json::json!({
            "type": "workspace_list",
            "workspaces": [
                {"workspace_id": "ws-1", "label": "main"}
            ]
        }));

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["workspace_id"], "ws-1");
    }

    #[test]
    fn layout_preset_maps_to_new_tab_apply_request() {
        let preset = LayoutPreset {
            id: "dual".into(),
            name: "Dual".into(),
            tab_label: None,
            workspace_id: Some("ws-1".into()),
            focus: true,
            root: acex_model::LayoutNode::Split {
                direction: acex_model::SplitDirection::Right,
                ratio: 0.5,
                first: Box::new(acex_model::LayoutNode::Pane {
                    command: None,
                    cwd: None,
                    env: Default::default(),
                    label: Some("left".into()),
                    pane_id: None,
                }),
                second: Box::new(acex_model::LayoutNode::Pane {
                    command: Some(vec!["cargo".into(), "test".into()]),
                    cwd: Some("crates".into()),
                    env: Default::default(),
                    label: Some("right".into()),
                    pane_id: None,
                }),
            },
        };

        let req = layout_apply_request(&preset).expect("layout apply request");

        assert_eq!(req.tab_label, Some("Dual"));
        assert_eq!(req.workspace_id, Some("ws-1"));
        assert!(req.focus);
        assert_eq!(req.root["type"], "split");
        assert_eq!(req.root["direction"], "right");
        assert_eq!(
            req.root["second"]["command"],
            serde_json::json!(["cargo", "test"])
        );
    }

    #[test]
    fn zed_diff_path_validation_rejects_blank_and_missing_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = dir.path().join("old.txt");
        std::fs::write(&old, "old").expect("write old");

        let blank = resolve_diff_paths("  ", old.to_str().expect("utf8 path"))
            .expect_err("blank old path should fail");
        assert!(blank.to_string().contains("zed diff old path is blank"));

        let missing = dir.path().join("missing.txt");
        let err = resolve_diff_paths(
            old.to_str().expect("utf8 path"),
            missing.to_str().expect("utf8 path"),
        )
        .expect_err("missing new path should fail");
        assert!(err.to_string().contains("zed diff new path does not exist"));
    }

    #[tokio::test]
    async fn diff_intent_validates_paths_and_calls_editor_bridge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = dir.path().join("old.txt");
        let new = dir.path().join("new.txt");
        std::fs::write(&old, "old").expect("write old");
        std::fs::write(&new, "new").expect("write new");
        let editor = RecordingEditor::default();
        let store = Arc::new(Mutex::new(Store::default()));

        handle_intent(
            &SocketTarget::Default,
            false,
            80,
            &editor,
            &store,
            Intent::DiffZed {
                old: old.to_string_lossy().into_owned(),
                new: new.to_string_lossy().into_owned(),
            },
        )
        .await
        .expect("diff intent");

        let diffs = editor.diffs.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].0, old);
        assert_eq!(diffs[0].1, new);
        drop(diffs);

        let store = lock_store(store.as_ref());
        assert_eq!(store.toast.as_deref(), Some("zed --diff"));
        assert_eq!(store.last_error, None);
    }
}
