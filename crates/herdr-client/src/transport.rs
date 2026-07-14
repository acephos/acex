//! Transport trait + mock + platform implementation (Unix UDS + Windows named pipe).

use async_trait::async_trait;

use crate::ndjson::{read_line, write_line};
use crate::{ClientError, Result, SocketTarget};

/// Abstract byte-level NDJSON transport.
///
/// Implementations: Unix domain socket, Windows named pipe, mock tests.
#[async_trait]
pub trait Transport: Send {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    /// Send one NDJSON request line; receive one response line.
    async fn call_ndjson(&mut self, request: &[u8]) -> Result<Vec<u8>>;
}

/// In-memory transport for tests and offline UI shell.
pub struct MockTransport {
    pub connected: bool,
    pub scripted: Vec<Vec<u8>>,
    idx: usize,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            connected: false,
            scripted: Vec::new(),
            idx: 0,
        }
    }

    pub fn with_responses(responses: Vec<Vec<u8>>) -> Self {
        Self {
            connected: false,
            scripted: responses,
            idx: 0,
        }
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn connect(&mut self) -> Result<()> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    async fn call_ndjson(&mut self, _request: &[u8]) -> Result<Vec<u8>> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }
        if self.idx >= self.scripted.len() {
            return Ok(br#"{"id":"0","result":{"type":"pong","protocol":16}}"#.to_vec());
        }
        let r = self.scripted[self.idx].clone();
        self.idx += 1;
        Ok(r)
    }
}

/// Convert a Herdr socket path into the Windows named-pipe path.
///
/// Herdr (via `interprocess`) exposes the filesystem path as:
/// `\\.\pipe\{absolute_path}` e.g.
/// `\\.\pipe\C:\Users\…\AppData\Roaming\herdr\herdr.sock`
#[cfg(windows)]
pub fn windows_pipe_path(socket_path: &std::path::Path) -> String {
    let abs = if socket_path.is_absolute() {
        socket_path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(socket_path)
    };
    format!(r"\\.\pipe\{}", abs.display())
}

pub mod platform {
    use super::*;

    /// Platform socket transport — Unix UDS or Windows named pipe.
    pub struct PlatformTransport {
        target: SocketTarget,
        #[cfg(unix)]
        stream: Option<tokio::net::UnixStream>,
        #[cfg(windows)]
        pipe: Option<tokio::net::windows::named_pipe::NamedPipeClient>,
    }

    impl PlatformTransport {
        pub fn new(target: SocketTarget) -> Self {
            Self {
                target,
                #[cfg(unix)]
                stream: None,
                #[cfg(windows)]
                pipe: None,
            }
        }

        pub fn target(&self) -> &SocketTarget {
            &self.target
        }

        fn path(&self) -> Result<std::path::PathBuf> {
            self.target
                .path_hint()
                .ok_or_else(|| ClientError::Message("no socket path".into()))
        }

        /// Write one NDJSON line on the open connection (subscribe path).
        pub async fn write_line(&mut self, data: &[u8]) -> Result<()> {
            #[cfg(unix)]
            {
                let stream = self.stream.as_mut().ok_or(ClientError::NotConnected)?;
                return write_line(stream, data).await;
            }
            #[cfg(windows)]
            {
                let pipe = self.pipe.as_mut().ok_or(ClientError::NotConnected)?;
                return write_line(pipe, data).await;
            }
            #[cfg(not(any(unix, windows)))]
            {
                let _ = data;
                Err(ClientError::Message("unsupported OS".into()))
            }
        }

        /// Read one NDJSON line from the open connection (subscribe path).
        pub async fn read_line(&mut self) -> Result<Vec<u8>> {
            #[cfg(unix)]
            {
                let stream = self.stream.as_mut().ok_or(ClientError::NotConnected)?;
                return read_line(stream).await;
            }
            #[cfg(windows)]
            {
                let pipe = self.pipe.as_mut().ok_or(ClientError::NotConnected)?;
                return read_line(pipe).await;
            }
            #[cfg(not(any(unix, windows)))]
            {
                Err(ClientError::Message("unsupported OS".into()))
            }
        }
    }

    #[async_trait]
    impl Transport for PlatformTransport {
        async fn connect(&mut self) -> Result<()> {
            let path = self.path()?;

            #[cfg(unix)]
            {
                let stream = tokio::net::UnixStream::connect(&path).await.map_err(|e| {
                    ClientError::Message(format!("unix connect {}: {e}", path.display()))
                })?;
                self.stream = Some(stream);
                return Ok(());
            }

            #[cfg(windows)]
            {
                use tokio::net::windows::named_pipe::ClientOptions;

                let pipe_path = super::windows_pipe_path(&path);
                // Herdr may take a moment after spawn; retry briefly on ERROR_FILE_NOT_FOUND / busy.
                let mut last_err = None;
                for attempt in 0..25u32 {
                    match ClientOptions::new().read(true).write(true).open(&pipe_path) {
                        Ok(client) => {
                            self.pipe = Some(client);
                            return Ok(());
                        }
                        Err(e) => {
                            last_err = Some(e);
                            // 2 = FILE_NOT_FOUND, 231 = ERROR_PIPE_BUSY
                            tokio::time::sleep(std::time::Duration::from_millis(
                                40 + u64::from(attempt) * 20,
                            ))
                            .await;
                        }
                    }
                }
                return Err(ClientError::Message(format!(
                    "windows named pipe connect {} (from {}): {}",
                    pipe_path,
                    path.display(),
                    last_err
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "unknown".into())
                )));
            }

            #[cfg(not(any(unix, windows)))]
            {
                let _ = path;
                Err(ClientError::Message(
                    "PlatformTransport: unsupported OS".into(),
                ))
            }
        }

        async fn disconnect(&mut self) -> Result<()> {
            #[cfg(unix)]
            {
                self.stream = None;
            }
            #[cfg(windows)]
            {
                self.pipe = None;
            }
            Ok(())
        }

        async fn call_ndjson(&mut self, request: &[u8]) -> Result<Vec<u8>> {
            #[cfg(unix)]
            {
                let stream = self.stream.as_mut().ok_or(ClientError::NotConnected)?;
                write_line(stream, request).await?;
                return read_line(stream).await;
            }

            #[cfg(windows)]
            {
                let pipe = self.pipe.as_mut().ok_or(ClientError::NotConnected)?;
                write_line(pipe, request).await?;
                return read_line(pipe).await;
            }

            #[cfg(not(any(unix, windows)))]
            {
                let _ = request;
                Err(ClientError::Message(
                    "PlatformTransport call not available on this OS".into(),
                ))
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests_win {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn pipe_path_uses_full_path_suffix() {
        let p = PathBuf::from(r"C:\Users\me\AppData\Roaming\herdr\herdr.sock");
        let pipe = windows_pipe_path(&p);
        assert_eq!(
            pipe,
            r"\\.\pipe\C:\Users\me\AppData\Roaming\herdr\herdr.sock"
        );
    }
}
