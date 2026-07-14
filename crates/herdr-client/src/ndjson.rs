//! Shared NDJSON line framing over any Tokio AsyncRead + AsyncWrite stream.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{ClientError, Result};

pub async fn write_line<W: AsyncWrite + Unpin>(w: &mut W, request: &[u8]) -> Result<()> {
    w.write_all(request).await?;
    if !request.ends_with(b"\n") {
        w.write_all(b"\n").await?;
    }
    w.flush().await?;
    Ok(())
}

pub async fn read_line<R: AsyncRead + Unpin>(r: &mut R) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        let n = r.read(&mut byte).await?;
        if n == 0 {
            return Err(ClientError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "eof from herdr socket",
            )));
        }
        buf.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
        // Guard runaway lines (schema responses can be large; 64 MiB ceiling).
        if buf.len() > 64 * 1024 * 1024 {
            return Err(ClientError::Message("ndjson line exceeds 64 MiB".into()));
        }
    }
    Ok(buf)
}
