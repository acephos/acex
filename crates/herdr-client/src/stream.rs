//! Long-lived NDJSON stream (subscribe path). Separate from unary connect-per-call.

use crate::resolve::SocketTarget;
use crate::transport::platform::PlatformTransport;
use crate::{Result, Transport};

/// Owns an open platform connection for multi-line protocols (`events.subscribe`).
pub struct NdjsonStream {
    inner: PlatformTransport,
}

impl NdjsonStream {
    pub async fn connect(target: SocketTarget) -> Result<Self> {
        let mut inner = PlatformTransport::new(target);
        inner.connect().await?;
        Ok(Self { inner })
    }

    pub async fn write_line(&mut self, data: &[u8]) -> Result<()> {
        self.inner.write_line(data).await
    }

    pub async fn read_line(&mut self) -> Result<Vec<u8>> {
        self.inner.read_line().await
    }

    pub async fn close(mut self) -> Result<()> {
        self.inner.disconnect().await
    }
}
