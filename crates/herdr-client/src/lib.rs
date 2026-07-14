//! Herdr NDJSON client.
//!
//! Platform IO is isolated behind [`Transport`]. Domain logic stays OS-agnostic.
//!
//! - **Unix:** domain socket at the configured path
//! - **Windows:** named pipe `\\.\pipe\{absolute_sock_path}` (Herdr / interprocess convention)

use std::path::PathBuf;
use std::time::Duration;

use herdr_types::{Request, Response, SessionSnapshot};
use thiserror::Error;
use tracing::{debug, info, warn};

pub mod ndjson;
pub mod ops;
pub mod resolve;
pub mod stream;
pub mod subscribe;
pub mod transport;

pub use ops::{extract_agent_rows, extract_read_text};
pub use resolve::{default_socket_path, resolve_socket_path, SocketTarget};
pub use stream::NdjsonStream;
pub use subscribe::{
    default_lifecycle_subscriptions, default_subscriptions_with_panes, EventSubscription,
    SubscriptionPush,
};
pub use transport::{MockTransport, Transport};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("rpc {code}: {message}")]
    Rpc { code: String, message: String },
    #[error("not connected")]
    NotConnected,
    #[error("timeout")]
    Timeout,
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, ClientError>;

/// High-level client over an abstract transport.
pub struct HerdrClient<T: Transport> {
    transport: T,
    connected: bool,
}

impl<T: Transport> HerdrClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            connected: false,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub async fn connect(&mut self) -> Result<()> {
        self.transport.connect().await?;
        self.connected = true;
        info!("herdr-client connected");
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        self.transport.disconnect().await?;
        self.connected = false;
        Ok(())
    }

    pub async fn request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        // Unary RPC uses a fresh connection per call.
        // On Windows, Herdr closes the named pipe after each response; Unix also
        // accepts connect-per-call. Long-lived `events.subscribe` will use a
        // dedicated stream (F03), not this path.
        self.ensure_fresh_connection().await?;

        let id = uuid::Uuid::new_v4().to_string();
        let req = Request {
            id: id.clone(),
            method: method.to_string(),
            params: params.unwrap_or(serde_json::json!({})),
        };
        let raw = serde_json::to_vec(&req)?;
        debug!(%method, %id, "rpc request");
        let resp_bytes = match self.transport.call_ndjson(&raw).await {
            Ok(b) => b,
            Err(e) => {
                // One reconnect retry on broken pipe / mid-call drop.
                warn!(error = %e, %method, "rpc transport error; retry once");
                self.ensure_fresh_connection().await?;
                self.transport.call_ndjson(&raw).await?
            }
        };

        // Drop connection after unary response (server may already have closed).
        let _ = self.transport.disconnect().await;
        self.connected = false;

        let resp: Response = serde_json::from_slice(&resp_bytes)?;
        if let Some(err) = resp.error {
            return Err(ClientError::Rpc {
                code: err.code,
                message: err.message,
            });
        }
        Ok(resp.result.unwrap_or(serde_json::Value::Null))
    }

    async fn ensure_fresh_connection(&mut self) -> Result<()> {
        let _ = self.transport.disconnect().await;
        self.connected = false;
        self.transport.connect().await?;
        self.connected = true;
        Ok(())
    }

    pub async fn ping(&mut self) -> Result<serde_json::Value> {
        self.request("ping", None).await
    }

    /// `session.snapshot` — unwraps `{ type, snapshot }` envelope when present.
    pub async fn session_snapshot(&mut self) -> Result<SessionSnapshot> {
        let v = self.request("session.snapshot", None).await?;
        parse_session_snapshot(v)
    }
}

/// Parse Herdr `session.snapshot` result (with or without envelope).
pub fn parse_session_snapshot(v: serde_json::Value) -> Result<SessionSnapshot> {
    if let Some(inner) = v.get("snapshot") {
        return Ok(serde_json::from_value(inner.clone())?);
    }
    Ok(serde_json::from_value(v)?)
}

/// Attempt connect; if socket missing, optionally spawn `herdr server` (best-effort).
pub async fn connect_with_optional_spawn(
    target: &SocketTarget,
    spawn_if_missing: bool,
) -> Result<HerdrClient<transport::platform::PlatformTransport>> {
    let mut client = HerdrClient::new(transport::platform::PlatformTransport::new(target.clone()));
    match client.connect().await {
        Ok(()) => Ok(client),
        Err(e) if spawn_if_missing => {
            warn!(error = %e, "connect failed; attempting herdr server spawn");
            spawn_herdr_server(target).await?;
            wait_until_connectable(&mut client, 40).await?;
            Ok(client)
        }
        Err(e) => Err(e),
    }
}

async fn wait_until_connectable(
    client: &mut HerdrClient<transport::platform::PlatformTransport>,
    attempts: u32,
) -> Result<()> {
    let mut last = None;
    for attempt in 0..attempts {
        tokio::time::sleep(Duration::from_millis(50 + u64::from(attempt) * 25)).await;
        match client.connect().await {
            Ok(()) => return Ok(()),
            Err(err) => last = Some(err),
        }
    }
    Err(last
        .unwrap_or_else(|| ClientError::Message("spawned herdr but connect still failed".into())))
}

/// Unary health + full session bootstrap (used on startup and F04 resync).
pub async fn ping_and_snapshot(
    target: &SocketTarget,
    spawn_if_missing: bool,
) -> Result<(serde_json::Value, SessionSnapshot)> {
    let mut client = connect_with_optional_spawn(target, spawn_if_missing).await?;
    let pong = client.ping().await?;
    let snap = client.session_snapshot().await?;
    let _ = client.disconnect().await;
    Ok((pong, snap))
}

/// Retry `ping_and_snapshot` with exponential backoff (F04 recovery).
pub async fn resync_with_backoff(
    target: &SocketTarget,
    spawn_if_missing: bool,
    max_attempts: u32,
) -> Result<(serde_json::Value, SessionSnapshot)> {
    let mut backoff_ms = 150u64;
    let mut last = None;
    for attempt in 0..max_attempts {
        match ping_and_snapshot(target, spawn_if_missing).await {
            Ok(v) => {
                if attempt > 0 {
                    info!(attempt, "resync succeeded");
                }
                return Ok(v);
            }
            Err(e) => {
                warn!(attempt, error = %e, backoff_ms, "resync attempt failed");
                last = Some(e);
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms.saturating_mul(2)).min(4_000);
            }
        }
    }
    Err(last.unwrap_or_else(|| ClientError::Message("resync exhausted".into())))
}

async fn spawn_herdr_server(target: &SocketTarget) -> Result<()> {
    // Use std::process so Windows creation_flags are available; fire-and-forget.
    let mut cmd = std::process::Command::new("herdr");
    cmd.arg("server");
    if let Some(path) = target.path_hint() {
        cmd.env("HERDR_SOCKET_PATH", &path);
    }
    if let Some(name) = target.session_name() {
        cmd.env("HERDR_SESSION", name);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // Detach: don't wait; server is long-lived (SOUL: outlives acex).
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }

    let child = cmd
        .spawn()
        .map_err(|e| ClientError::Message(format!("failed to spawn herdr: {e}")))?;
    debug!(pid = ?child.id(), "spawned herdr server process");
    // Drop Child without wait — leave server running.
    std::mem::forget(child);
    Ok(())
}

/// Config knobs shared with acex-config later.
#[derive(Debug, Clone, Default)]
pub struct ClientConfig {
    pub socket: Option<PathBuf>,
    pub session: Option<String>,
    pub spawn_if_missing: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn snapshot_envelope() {
        let v = json!({
            "type": "session_snapshot",
            "snapshot": {
                "protocol": 16,
                "agents": [{"id": "a", "state": "idle"}],
                "workspaces": []
            }
        });
        let snap = parse_session_snapshot(v).unwrap();
        assert_eq!(snap.agents.len(), 1);
        assert_eq!(snap.agents[0].id, "a");
    }

    #[tokio::test]
    async fn mock_ping() {
        let mut c = HerdrClient::new(MockTransport::new());
        c.connect().await.unwrap();
        let v = c.ping().await.unwrap();
        assert_eq!(v["type"], "pong");
    }

    #[tokio::test]
    async fn mock_unary_agent_focus() {
        // Scripted success for agent.focus — exercises shipped HerdrClient::request path.
        let body = br#"{"id":"x","result":{"type":"agent_focused","target":"w1:p1"}}"#.to_vec();
        let mut c = HerdrClient::new(MockTransport::with_responses(vec![body]));
        c.connect().await.unwrap();
        let v = c.agent_focus("w1:p1").await.expect("focus");
        assert_eq!(v["type"], "agent_focused");
        assert_eq!(v["target"], "w1:p1");
    }

    #[tokio::test]
    async fn mock_rpc_error_surfaces() {
        let body = br#"{"id":"x","error":{"code":"not_found","message":"pane missing"}}"#.to_vec();
        let mut c = HerdrClient::new(MockTransport::with_responses(vec![body]));
        c.connect().await.unwrap();
        let err = c.agent_get("nope").await.expect_err("must fail");
        match err {
            ClientError::Rpc { code, message } => {
                assert_eq!(code, "not_found");
                assert!(message.contains("pane"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn extract_read_text_prefers_text_field() {
        let v = json!({"text": "hello\nworld", "revision": 1});
        assert_eq!(extract_read_text(&v), "hello\nworld");
        let nested = json!({"read": {"text": "nested"}});
        assert_eq!(extract_read_text(&nested), "nested");
    }

    #[test]
    fn extract_agent_rows_from_envelope() {
        let v = json!({"agents": [{"pane_id": "w1:p1"}, {"pane_id": "w1:p2"}]});
        assert_eq!(extract_agent_rows(&v).len(), 2);
        let arr = json!([{"pane_id": "a"}]);
        assert_eq!(extract_agent_rows(&arr).len(), 1);
    }
}
