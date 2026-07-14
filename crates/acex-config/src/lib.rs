//! Config for acex — files optional; env + defaults always work.

use std::path::{Path, PathBuf};

use acex_model::StartPreset;
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
    /// Agent start presets loaded from config.toml.
    pub start_presets: Vec<StartPreset>,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    socket_path: Option<PathBuf>,
    session: Option<String>,
    spawn_herdr_if_missing: Option<bool>,
    leave_server_on_exit: Option<bool>,
    editor_bin: Option<String>,
    peek_lines: Option<u32>,
    #[serde(default)]
    start_presets: Vec<StartPreset>,
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
            start_presets: Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        Self::load_from_dir(Self::config_dir())
    }

    pub fn load_from_dir(config_dir: impl AsRef<Path>) -> Self {
        let mut c = Self::default();
        c.apply_file_config(config_dir.as_ref().join("config.toml"));
        c.apply_env();
        c
    }

    fn apply_file_config(&mut self, path: PathBuf) {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(file) = toml::from_str::<FileConfig>(&raw) else {
            return;
        };

        if let Some(socket_path) = file.socket_path {
            self.socket_path = Some(socket_path);
        }
        if let Some(session) = non_empty(file.session) {
            self.session = Some(session);
        }
        if let Some(spawn) = file.spawn_herdr_if_missing {
            self.spawn_herdr_if_missing = spawn;
        }
        if let Some(leave) = file.leave_server_on_exit {
            self.leave_server_on_exit = leave;
        }
        if let Some(editor_bin) = non_empty(file.editor_bin) {
            self.editor_bin = editor_bin;
        }
        if let Some(peek_lines) = file.peek_lines {
            self.peek_lines = peek_lines;
        }
        self.start_presets = file.start_presets;
    }

    fn apply_env(&mut self) {
        if let Ok(p) = std::env::var("HERDR_SOCKET_PATH") {
            if !p.is_empty() {
                self.socket_path = Some(PathBuf::from(p));
            }
        }
        if let Ok(s) = std::env::var("HERDR_SESSION") {
            if !s.is_empty() {
                self.session = Some(s);
            }
        }
        if let Ok(e) = std::env::var("ACEX_EDITOR") {
            if !e.is_empty() {
                self.editor_bin = e;
            }
        }
    }

    pub fn config_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("ACEX_CONFIG_DIR") {
            if !dir.is_empty() {
                return PathBuf::from(dir);
            }
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("acex")
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loads_start_presets_from_config_toml() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            r#"
                [[start_presets]]
                id = "review"
                name = "reviewer"
                argv = ["omp", "--agent", "reviewer"]
                cwd = "crates"
            "#,
        )
        .unwrap();

        let cfg = Config::load_from_dir(dir.path());

        assert_eq!(cfg.start_presets.len(), 1);
        assert_eq!(cfg.start_presets[0].id, "review");
        assert_eq!(cfg.start_presets[0].name, "reviewer");
        assert_eq!(cfg.start_presets[0].argv, ["omp", "--agent", "reviewer"]);
        assert_eq!(cfg.start_presets[0].cwd.as_deref(), Some("crates"));
    }

    #[test]
    fn missing_config_keeps_empty_presets() {
        let dir = tempdir().unwrap();
        let cfg = Config::load_from_dir(dir.path());

        assert!(cfg.start_presets.is_empty());
    }
}
