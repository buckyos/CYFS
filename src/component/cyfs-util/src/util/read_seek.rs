use async_std::io::{Read, Seek};
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait AsyncReadWithSeek: Read + Seek {}

impl AsyncReadWithSeek for async_std::fs::File {}
impl<T> AsyncReadWithSeek for async_std::io::Cursor<T> where T: AsRef<[u8]> + Unpin {}

// TODO just for trait upcasting
pub struct AsyncReadWithSeekAdapter {
    reader: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync + 'static>,
}

impl AsyncReadWithSeekAdapter {
    pub fn new(reader: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync + 'static>) -> Self {
        Self { reader }
    }

    pub fn into_reader(self) -> Box<dyn Read + Unpin + Send + Sync + 'static> {
        Box::new(self)
    }
}

impl Read for AsyncReadWithSeekAdapter {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.reader).poll_read(cx, buf)
    }
}
