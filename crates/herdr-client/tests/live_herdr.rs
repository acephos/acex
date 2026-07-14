//! Optional live E2E against a running Herdr server.
//!
//! ```text
//! HERDR_E2E=1 cargo test -p herdr-client --test live_herdr -- --nocapture
//! ```

use std::time::Duration;

use herdr_client::{
    connect_with_optional_spawn, default_lifecycle_subscriptions, resolve_socket_path,
    EventSubscription,
};

fn e2e_enabled() -> bool {
    std::env::var("HERDR_E2E").ok().as_deref() == Some("1")
}

#[tokio::test]
async fn ping_and_snapshot_live() {
    if !e2e_enabled() {
        eprintln!("skip live_herdr (set HERDR_E2E=1)");
        return;
    }

    let target = resolve_socket_path(None, None);
    let mut client = connect_with_optional_spawn(&target, true)
        .await
        .expect("connect");
    let pong = client.ping().await.expect("ping");
    assert_eq!(pong["type"], "pong");
    let protocol = pong["protocol"].as_u64().unwrap_or(0);
    assert!(protocol >= 16, "protocol={protocol}");

    let snap = client.session_snapshot().await.expect("snapshot");
    eprintln!(
        "snapshot workspaces={} panes={} agents={} protocol={:?}",
        snap.workspaces.len(),
        snap.panes.len(),
        snap.agents.len(),
        snap.protocol
    );
}

#[tokio::test]
async fn subscribe_live() {
    if !e2e_enabled() {
        eprintln!("skip subscribe_live (set HERDR_E2E=1)");
        return;
    }

    let target = resolve_socket_path(None, None);
    let mut sub = EventSubscription::start(target, default_lifecycle_subscriptions())
        .await
        .expect("subscribe");

    // Trigger a lifecycle event via CLI.
    let _ = std::process::Command::new("herdr")
        .args(["workspace", "create", "--label", "acex-e2e-sub"])
        .status();

    let push = tokio::time::timeout(Duration::from_secs(5), sub.next_event())
        .await
        .expect("timeout waiting event")
        .expect("event");
    eprintln!("got event={}", push.event);
    assert!(
        push.event.contains("workspace")
            || push.event.contains("tab")
            || push.event.contains("pane"),
        "unexpected event {}",
        push.event
    );
    let _ = sub.close().await;
}
