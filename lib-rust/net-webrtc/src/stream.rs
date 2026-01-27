use std::io::Result;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf};
use tracing::{debug, error};
use webrtc::data::data_channel::DataChannel as DetachedDataChannel;

/// A wrapper around WebRTC DetachedDataChannel that implements
/// tokio::io::AsyncRead and tokio::io::AsyncWrite.
///
/// Since DetachedDataChannel exposes an async API (not poll-based) and is message-oriented,
/// this wrapper spawns a background task to bridge the data between the channel and
/// a tokio::io::duplex stream.
pub struct WebRTCStream {
    inner: DuplexStream,
}

impl WebRTCStream {
    pub fn new(channel: Arc<DetachedDataChannel>) -> Self {
        let (local, mut remote) = tokio::io::duplex(65536); // 64KB buffer

        tokio::spawn(async move {
            let mut buf_in = vec![0u8; 8192]; // Buffer for incoming WebRTC data
            let mut buf_out = vec![0u8; 8192]; // Buffer for outgoing WebRTC data

            loop {
                tokio::select! {
                    // Read from WebRTC -> Write to Duplex (to be read by user)
                    res = channel.read(&mut buf_in) => {
                        match res {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                if let Err(e) = remote.write_all(&buf_in[..n]).await {
                                    debug!("WebRTCStream bridge: failed to write to duplex: {}", e);
                                    break;
                                }
                            },
                            Err(e) => {
                                error!("WebRTCStream bridge: WebRTC read error: {}", e);
                                break;
                            }
                        }
                    },
                    // Read from Duplex (written by user) -> Write to WebRTC
                    res = remote.read(&mut buf_out) => {
                        match res {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                let data = bytes::Bytes::copy_from_slice(&buf_out[..n]);
                                if let Err(e) = channel.write(&data).await {
                                    error!("WebRTCStream bridge: WebRTC write error: {}", e);
                                    break;
                                }
                            },
                            Err(e) => {
                                debug!("WebRTCStream bridge: duplex read error: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
            debug!("WebRTCStream bridge task finished");
            // Channel will be dropped here, closing the connection
        });

        Self { inner: local }
    }
}

impl AsyncRead for WebRTCStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for WebRTCStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
