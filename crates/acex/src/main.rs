//! acex — Herdr-centric agent control plane.
//!
//! See SOUL.md, GOAL.md, AGENTS.md, docs/tracker.html.

mod checkpoint_status;
mod sync_util;
mod worker;

use checkpoint_status::{build_checkpoint_status, collect_git_info};
use sync_util::lock_store;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acex_config::Config;
use acex_discover::{scan, DiscoveryReport};
use acex_model::{ConnState, Intent, Store};
use acex_ui::App;
use herdr_client::{
    connect_with_optional_spawn, default_subscriptions_with_panes, extract_agent_rows,
    ping_and_snapshot, resolve_socket_path, resync_with_backoff, EventSubscription, SocketTarget,
};
use herdr_types::Event;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let offline = args.iter().any(|a| a == "--offline");
    let smoke = args.iter().any(|a| a == "--smoke");
    let smoke_reconnect = args.iter().any(|a| a == "--smoke-reconnect");
    // Machine-readable discovery + connection summary (Pi-like JSON surface).
    let status = args.iter().any(|a| a == "--status");
    let checkpoint_status = args.iter().any(|a| a == "--checkpoint-status");

    if !checkpoint_status {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_writer(std::io::stderr)
            .init();
    }

    let cfg = Config::load();

    info!(
        editor = %cfg.editor_bin,
        spawn = cfg.spawn_herdr_if_missing,
        leave_server = cfg.leave_server_on_exit,
        offline,
        smoke,
        smoke_reconnect,
        status,
        "acex starting"
    );

    let mut store = Store {
        conn: ConnState::Connecting,
        peek_line_limit: cfg.peek_lines,
        ..Default::default()
    };

    let target = resolve_socket_path(cfg.socket_path.clone(), cfg.session.clone());
    let spawn = cfg.spawn_herdr_if_missing;

    if checkpoint_status {
        let discovery = discover_project();
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let body =
            build_checkpoint_status(&root, &target, &discovery, collect_git_info(&root), &cfg);
        println!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".into())
        );
        return Ok(());
    }

    if offline {
        store.conn = ConnState::Offline;
        store.set_error("offline mode (--offline); no Herdr connect".to_string());
    } else {
        info!(?target, path = ?target.path_hint(), "resolving herdr socket");
        match bootstrap(&target, spawn, &mut store).await {
            Ok(()) => {}
            Err(e) => {
                error!(error = %e, "bootstrap failed");
                store.conn = ConnState::Offline;
                store.set_error(format!(
                    "offline · {e} · start `herdr server` or check HERDR_SOCKET_PATH"
                ));
            }
        }
    }

    let discovery = discover_project();

    if status {
        print_status_json(&store, &discovery, offline, &cfg);
        // --status never fails solely because Herdr is down; discovery still emits.
        return Ok(());
    }

    if smoke || smoke_reconnect {
        if store.conn == ConnState::Live && !offline {
            if let Err(e) = smoke_subscribe(&target, &mut store).await {
                warn!(error = %e, "smoke subscribe skipped/failed");
            }
            if let Err(e) = smoke_phase1(&target, spawn, &mut store).await {
                warn!(error = %e, "smoke phase1 partial");
            }
        }
        if smoke_reconnect && !offline {
            if let Err(e) = smoke_f04_reconnect(&target, spawn, &mut store).await {
                error!(error = %e, "smoke-reconnect failed");
                anyhow::bail!("smoke-reconnect failed: {e}");
            }
        }
        println!(
            "acex smoke · conn={:?} sub={} ws={} panes={} agents={} events={} reconnects={} snap_gen={} peek_lines={} packages={} skills={} err={:?}",
            store.conn,
            store.subscribed,
            store.workspace_count,
            store.pane_count,
            store.agents.len(),
            store.event_count,
            store.reconnect_count,
            store.snapshot_gen,
            store.peek_lines.len(),
            discovery.packages.len(),
            discovery.skills.len(),
            store.last_error
        );
        // Stable parseable block (Pi-like machine mode).
        print_status_json(&store, &discovery, offline, &cfg);
        if store.conn != ConnState::Live && !offline {
            anyhow::bail!("smoke failed: not live");
        }
        if smoke_reconnect && store.reconnect_count == 0 && !offline {
            anyhow::bail!("smoke-reconnect failed: reconnect_count still 0");
        }
        info!("acex smoke exit (Herdr left running if it was)");
        return Ok(());
    }

    let store = Arc::new(Mutex::new(store));
    let quit = Arc::new(AtomicBool::new(false));
    let (intent_tx, intent_rx) = mpsc::channel::<Intent>();

    if !offline {
        let store_ev = Arc::clone(&store);
        let quit_ev = Arc::clone(&quit);
        let target_ev = target.clone();
        tokio::spawn(async move {
            run_live_loop(target_ev, spawn, store_ev, quit_ev).await;
        });

        let store_w = Arc::clone(&store);
        let target_w = target.clone();
        let editor_bin = cfg.editor_bin.clone();
        let peek_lines = cfg.peek_lines;
        tokio::spawn(async move {
            worker::run_intent_worker(target_w, spawn, editor_bin, peek_lines, store_w, intent_rx)
                .await;
        });
    } else {
        // Drop receiver side unused offline — UI can still open palette offline.
        drop(intent_rx);
    }

    let app =
        App::with_shared_and_presets(Arc::clone(&store), intent_tx, cfg.start_presets.clone());
    let quit_ui = Arc::clone(&quit);
    let ui = tokio::task::spawn_blocking(move || {
        let r = acex_ui::run(app);
        quit_ui.store(true, Ordering::SeqCst);
        r
    });

    ui.await.map_err(|e| anyhow::anyhow!("ui join: {e}"))??;

    info!("acex exit (Herdr left running if it was)");
    Ok(())
}

/// Discover packages/skills from cwd (project root for drop-in agents).
fn discover_project() -> DiscoveryReport {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match scan(&root) {
        Ok(r) => {
            info!(
                packages = r.packages.len(),
                skills = r.skills.len(),
                root = %root.display(),
                "discovery scan"
            );
            r
        }
        Err(e) => {
            warn!(error = %e, "discovery scan failed");
            DiscoveryReport::default()
        }
    }
}

fn print_status_json(store: &Store, discovery: &DiscoveryReport, offline_flag: bool, cfg: &Config) {
    let conn = match store.conn {
        ConnState::Offline => "Offline",
        ConnState::Connecting => "Connecting",
        ConnState::Live => "Live",
        ConnState::Reconnecting => "Reconnecting",
    };
    let packages: Vec<serde_json::Value> = discovery
        .packages
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name.as_str(),
                "description": p.description.as_str(),
                "path": p.path.to_string_lossy(),
                "source": p.source,
            })
        })
        .collect();
    let skills: Vec<serde_json::Value> = discovery
        .skills
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name.as_str(),
                "description": s.description.as_str(),
                "path": s.path.to_string_lossy(),
            })
        })
        .collect();
    let diagnostics: Vec<serde_json::Value> = discovery
        .diagnostics
        .iter()
        .map(|d| {
            serde_json::json!({
                "severity": d.severity,
                "code": d.code.as_str(),
                "path": d.path.to_string_lossy(),
                "message": d.message.as_str(),
            })
        })
        .collect();
    let body = serde_json::json!({
        "acex_status": 1,
        "conn": conn,
        "offline_flag": offline_flag,
        "subscribed": store.subscribed,
        "workspaces": store.workspace_count,
        "panes": store.pane_count,
        "agents": store.agents.len(),
        "events": store.event_count,
        "reconnects": store.reconnect_count,
        "snapshot_gen": store.snapshot_gen,
        "error": store.last_error,
        "packages": packages.clone(),
        "skills": skills.clone(),
        "diagnostics": diagnostics.clone(),
        "discovery": {
            "packages": packages,
            "skills": skills,
            "diagnostics": diagnostics,
        },
        "config": {
            "start_presets": &cfg.start_presets
        },
        "seams": [
            "Intent",
            "Transport",
            "EditorBridge",
            "acex-discover"
        ],
    });
    println!("--- acex-status ---");
    println!(
        "{}",
        serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".into())
    );
    println!("--- end acex-status ---");
}

async fn bootstrap(target: &SocketTarget, spawn: bool, store: &mut Store) -> anyhow::Result<()> {
    let (pong, snap) = ping_and_snapshot(target, spawn).await?;
    info!(?pong, "ping ok");
    let ws = snap.workspaces.len();
    let panes = snap.panes.len();
    info!(ws, panes, "session.snapshot applied");
    store.apply_snapshot(snap);
    store.ensure_selection();

    // Best-effort agent.list enrichment.
    if let Ok(mut client) = connect_with_optional_spawn(target, spawn).await {
        if let Ok(list) = client.agent_list().await {
            let rows = extract_agent_rows(&list);
            if !rows.is_empty() {
                store.merge_agent_values(&rows);
            }
        }
        let _ = client.disconnect().await;
    }
    Ok(())
}

async fn smoke_subscribe(target: &SocketTarget, store: &mut Store) -> anyhow::Result<()> {
    let pane_ids = pane_ids_from_store(store);
    let subs = default_subscriptions_with_panes(&pane_ids);
    let mut sub = EventSubscription::start(target.clone(), subs).await?;
    store.mark_subscribed();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), sub.next_event()).await {
            Ok(Ok(push)) => {
                store.apply_event(&Event {
                    event: push.event,
                    data: push.data,
                    extra: Default::default(),
                });
            }
            Ok(Err(e)) => {
                warn!(error = %e, "smoke subscribe read error");
                break;
            }
            Err(_) => break,
        }
    }
    let _ = sub.close().await;
    store.subscribed = false;
    Ok(())
}

async fn smoke_phase1(target: &SocketTarget, spawn: bool, store: &mut Store) -> anyhow::Result<()> {
    store.ensure_selection();
    let Some(t) = store.selected_target() else {
        info!("smoke phase1: no target");
        return Ok(());
    };
    let lines = store.peek_line_limit.max(40);
    let mut client = connect_with_optional_spawn(target, spawn).await?;
    let text = match client.agent_read(&t, "recent", lines, true).await {
        Ok(v) => herdr_client::extract_read_text(&v),
        Err(e) => {
            warn!(error = %e, "smoke agent.read; trying pane.read");
            match client.pane_read(&t, "recent", lines, true).await {
                Ok(v) => herdr_client::extract_read_text(&v),
                Err(e2) => {
                    warn!(error = %e2, "smoke pane.read failed");
                    String::new()
                }
            }
        }
    };
    if !text.is_empty() {
        store.set_peek(&t, &text);
    }
    if let Ok(w) = client.worktree_list(None, None).await {
        store.worktrees = w
            .get("worktrees")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| serde_json::to_string(x).ok())
                    .collect()
            })
            .unwrap_or_default();
    }
    let _ = client.disconnect().await;
    Ok(())
}

async fn smoke_f04_reconnect(
    target: &SocketTarget,
    spawn: bool,
    store: &mut Store,
) -> anyhow::Result<()> {
    store.mark_reconnecting("smoke: herdr server stop");
    info!("smoke-reconnect: stopping herdr server");
    let _ = std::process::Command::new("herdr")
        .args(["server", "stop"])
        .status();
    tokio::time::sleep(Duration::from_millis(300)).await;

    info!("smoke-reconnect: resync_with_backoff (spawn={spawn})");
    let (pong, snap) = resync_with_backoff(target, spawn, 20).await?;
    info!(?pong, "smoke-reconnect ping ok");
    store.apply_resnapshot(snap);
    let pane_ids = pane_ids_from_store(store);
    let mut sub =
        EventSubscription::start(target.clone(), default_subscriptions_with_panes(&pane_ids))
            .await?;
    store.mark_subscribed();
    let _ = tokio::time::timeout(Duration::from_millis(200), sub.next_event()).await;
    let _ = sub.close().await;
    store.subscribed = false;
    store.last_error = None;
    store.status_note = Some(format!(
        "smoke-reconnect ok · reconnects={} · snap_gen={}",
        store.reconnect_count, store.snapshot_gen
    ));
    Ok(())
}

fn pane_ids_from_store(store: &Store) -> Vec<String> {
    store
        .snapshot
        .panes
        .iter()
        .filter_map(|p| {
            p.get("pane_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

async fn run_live_loop(
    target: SocketTarget,
    spawn: bool,
    store: Arc<Mutex<Store>>,
    quit: Arc<AtomicBool>,
) {
    let mut need_resnapshot = false;
    let mut backoff_ms = 200u64;

    while !quit.load(Ordering::SeqCst) {
        if need_resnapshot {
            {
                let mut s = lock_store(&store);
                s.mark_reconnecting("stream lost · resnapshot");
            }
            match resync_with_backoff(&target, spawn, 24).await {
                Ok((_pong, snap)) => {
                    let mut s = lock_store(&store);
                    s.apply_resnapshot(snap);
                    s.ensure_selection();
                    s.last_error = None;
                    info!(
                        reconnects = s.reconnect_count,
                        gen = s.snapshot_gen,
                        "F04 resnapshot applied"
                    );
                    backoff_ms = 200;
                }
                Err(e) => {
                    warn!(error = %e, "F04 resnapshot failed");
                    {
                        let mut s = lock_store(&store);
                        s.mark_reconnecting(format!("resync failed: {e}"));
                    }
                    if quit.load(Ordering::SeqCst) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(5_000);
                    continue;
                }
            }
        }

        let pane_ids = {
            let s = lock_store(&store);
            pane_ids_from_store(&s)
        };
        let subs = default_subscriptions_with_panes(&pane_ids);

        match EventSubscription::start(target.clone(), subs).await {
            Ok(mut sub) => {
                {
                    let mut s = lock_store(&store);
                    s.mark_subscribed();
                    s.last_error = None;
                    if s.reconnect_count > 0 {
                        s.status_note = Some(format!(
                            "live · resynced×{} · snap gen={}",
                            s.reconnect_count, s.snapshot_gen
                        ));
                    } else {
                        s.status_note = Some("events.subscribe · live".into());
                    }
                }
                need_resnapshot = true;
                backoff_ms = 200;
                info!("subscribe loop connected");

                loop {
                    if quit.load(Ordering::SeqCst) {
                        let _ = sub.close().await;
                        return;
                    }
                    match tokio::time::timeout(Duration::from_millis(500), sub.next_event()).await {
                        Ok(Ok(push)) => {
                            let mut s = lock_store(&store);
                            s.apply_event(&Event {
                                event: push.event,
                                data: push.data,
                                extra: Default::default(),
                            });
                        }
                        Ok(Err(e)) => {
                            warn!(error = %e, "subscribe stream ended");
                            let mut s = lock_store(&store);
                            s.mark_reconnecting(format!("subscribe dropped: {e}"));
                            break;
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "subscribe start failed");
                {
                    let mut s = lock_store(&store);
                    s.mark_reconnecting(format!("subscribe failed: {e}"));
                }
                need_resnapshot = true;
            }
        }

        if quit.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms.saturating_mul(2)).min(5_000);
    }
}
