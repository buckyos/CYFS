use std::pin::Pin;
use async_std::io::Read as AsyncRead;
use async_std::io::ReadExt;
use std::io::prelude::*;

struct ReadBridge {
    reader: Pin<Box<dyn AsyncRead + Send + Unpin + 'static>>,
}

impl Read for ReadBridge {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut reader = self.reader.as_mut();
        async_std::task::block_on(async {
            reader.read(buf).await
        })
    }
}

/// Bridge from AsyncRead to Read.
pub fn async_read_to_sync<S: AsyncRead + Unpin + Send + 'static>(
    reader: S,
) -> impl Read + Send + Unpin + 'static {
    let reader = Box::pin(reader);
    ReadBridge { reader }
}