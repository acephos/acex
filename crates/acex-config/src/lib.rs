//! Config for acex — files optional; env + defaults always work.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Optional explicit Herdr socket path.
    pub socket_path: Option<PathBuf>,
    /// Optional Herdr session name.
    pub session: Option<String>,
    /// Spawn `herdr server` when socket missing.
    pub spawn_herdr_if_missing: bool,
    /// Never stop Herdr server on acex quit (SOUL).
    pub leave_server_on_exit: bool,
    /// Editor binary (default `zed`).
    pub editor_bin: String,
    /// Peek default lines.
    pub peek_lines: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket_path: None,
            session: None,
            spawn_herdr_if_missing: true,
            leave_server_on_exit: true,
            editor_bin: "zed".into(),
            peek_lines: 80,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        // Phase 0: defaults + env. File config (TOML) lands with presets (F19).
        let mut c = Self::default();
        if let Ok(p) = std::env::var("HERDR_SOCKET_PATH") {
            if !p.is_empty() {
                c.socket_path = Some(PathBuf::from(p));
            }
        }
        if let Ok(s) = std::env::var("HERDR_SESSION") {
            if !s.is_empty() {
                c.session = Some(s);
            }
        }
        if let Ok(e) = std::env::var("ACEX_EDITOR") {
            if !e.is_empty() {
                c.editor_bin = e;
            }
        }
        c
    }

    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("acex")
    }
}
