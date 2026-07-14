//! Socket path resolution (platform-agnostic policy; OS details in Transport).

use std::path::PathBuf;

/// Where the Herdr control socket lives.
#[derive(Debug, Clone)]
pub enum SocketTarget {
    /// Explicit path (Unix domain socket path or Windows pipe name/path).
    Path(PathBuf),
    /// Named Herdr session (`HERDR_SESSION` / `…/sessions/<name>/herdr.sock`).
    Session(String),
    /// Default session location.
    Default,
}

impl SocketTarget {
    pub fn path_hint(&self) -> Option<PathBuf> {
        match self {
            Self::Path(p) => Some(p.clone()),
            Self::Session(name) => Some(session_socket_path(name)),
            Self::Default => Some(default_socket_path()),
        }
    }

    pub fn session_name(&self) -> Option<&str> {
        match self {
            Self::Session(n) => Some(n.as_str()),
            _ => None,
        }
    }
}

/// Resolve from env + optional overrides.
///
/// Order: explicit path → `HERDR_SOCKET_PATH` → `HERDR_SESSION` → default.
pub fn resolve_socket_path(explicit: Option<PathBuf>, session: Option<String>) -> SocketTarget {
    if let Some(p) = explicit {
        return SocketTarget::Path(p);
    }
    if let Ok(p) = std::env::var("HERDR_SOCKET_PATH") {
        if !p.is_empty() {
            return SocketTarget::Path(PathBuf::from(p));
        }
    }
    if let Some(s) = session {
        return SocketTarget::Session(s);
    }
    if let Ok(s) = std::env::var("HERDR_SESSION") {
        if !s.is_empty() {
            return SocketTarget::Session(s);
        }
    }
    SocketTarget::Default
}

/// Default socket: `~/.config/herdr/herdr.sock` (Unix convention; used as hint elsewhere).
pub fn default_socket_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("herdr")
        .join("herdr.sock")
}

pub fn session_socket_path(name: &str) -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("herdr")
        .join("sessions")
        .join(name)
        .join("herdr.sock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_wins() {
        let t = resolve_socket_path(Some(PathBuf::from("/tmp/x.sock")), Some("s".into()));
        match t {
            SocketTarget::Path(p) => assert_eq!(p, PathBuf::from("/tmp/x.sock")),
            _ => panic!("expected path"),
        }
    }
}
