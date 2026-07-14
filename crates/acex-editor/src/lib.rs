//! Editor bridge — extensibility pillar.
//!
//! Default adapter: Zed CLI. Other editors implement [`EditorBridge`].

use std::path::Path;
use std::process::Command;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EditorError {
    #[error("editor binary not found: {0}")]
    NotFound(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, EditorError>;

/// How to open a path.
#[derive(Debug, Clone, Copy)]
pub enum OpenMode {
    /// Default: `zed <path>`
    Default,
    /// New window: `zed -n`
    NewWindow,
    /// Add to window: `zed -a`
    AddToWindow,
}

/// Platform-agnostic editor operations.
pub trait EditorBridge: Send + Sync {
    fn open_path(&self, path: &Path, mode: OpenMode) -> Result<()>;
    fn diff(&self, old: &Path, new: &Path) -> Result<()>;
}

/// Zed CLI adapter.
#[derive(Debug, Clone)]
pub struct ZedBridge {
    pub bin: String,
}

impl Default for ZedBridge {
    fn default() -> Self {
        Self { bin: "zed".into() }
    }
}

impl ZedBridge {
    pub fn new(bin: impl Into<String>) -> Self {
        Self { bin: bin.into() }
    }
}

impl EditorBridge for ZedBridge {
    fn open_path(&self, path: &Path, mode: OpenMode) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        match mode {
            OpenMode::Default => {}
            OpenMode::NewWindow => {
                cmd.arg("-n");
            }
            OpenMode::AddToWindow => {
                cmd.arg("-a");
            }
        }
        cmd.arg(path);
        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(EditorError::NotFound(self.bin.clone()))
            }
            Err(e) => Err(e.into()),
        }
    }

    fn diff(&self, old: &Path, new: &Path) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--diff").arg(old).arg(new);
        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(EditorError::NotFound(self.bin.clone()))
            }
            Err(e) => Err(e.into()),
        }
    }
}
