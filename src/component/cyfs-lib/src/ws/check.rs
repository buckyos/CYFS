use cyfs_base::*;

use std::pin::Pin;
use std::task::{Context, Poll};
use async_std::net::TcpStream;
use std::sync::Arc;
use async_std::io::Cursor;

#[async_trait::async_trait]
pub trait WebSocketSessionChecker: Send + Sync {
    async fn check(&self, req: http_types::Request) -> BuckyResult<()>;
}

pub type WebSocketSessionCheckerRef = Arc<Box<dyn WebSocketSessionChecker>>;

const MAX_HEAD_LENGTH: usize = 8 * 1024;
const LF: u8 = b'\n';

#[derive(Clone)]
pub struct WebSocketPeekStream {
    req: Cursor<Vec<u8>>,
    stream: TcpStream,
}

impl WebSocketPeekStream {
    pub async fn new(stream: TcpStream) -> BuckyResult<Self> {
        let mut ret = Self {
            req: Cursor::new(vec![0; MAX_HEAD_LENGTH]),
            stream,
        };

        ret.read_req().await?;
        Ok(ret)
    }

    async fn read_req(&mut self) -> BuckyResult<()> {
        let conn_info = (
            self.stream.local_addr().unwrap(),
            self.stream.peer_addr().unwrap(),
        );

        loop {
            let buf = self.req.get_mut();
            let size = self.stream.peek(buf).await.map_err(|e| {
                let msg = format!("peek request from stream error! {:?}, {}", conn_info, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

            let mut got = false;
            for i in 0..size {
                if buf[i] == LF {
                    got = true;
                    break;
                }
            }

            if got {
                break;
            }
        }

        Ok(())
    }
}

impl async_std::io::Read for WebSocketPeekStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.req).poll_read(cx, buf)
    }
}

impl async_std::io::Write for WebSocketPeekStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_close(cx)
    }
}