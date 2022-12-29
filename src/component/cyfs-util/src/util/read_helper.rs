use super::read_seek::AsyncReadWithSeek;
use cyfs_base::*;

use cyfs_sha2 as sha2;
use futures::AsyncSeekExt;
use sha2::Digest;
use std::io::SeekFrom;
use std::ops::Range;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct ReaderWithLimit {
    limit: u64,
    range: Range<u64>,
    reader: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
}

impl ReaderWithLimit {
    pub async fn new(
        limit: u64,
        mut reader: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
    ) -> BuckyResult<Self> {
        let start = reader.stream_position().await?;
        let range = Range {
            start,
            end: start + limit,
        };

        Ok(Self {
            limit,
            range,
            reader,
        })
    }
}

impl async_std::io::Read for ReaderWithLimit {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.limit == 0 {
            return Poll::Ready(Ok(0));
        }

        let max = std::cmp::min(buf.len() as u64, self.limit) as usize;
        let ret = Pin::new(self.reader.as_mut()).poll_read(cx, &mut buf[..max]);
        match ret {
            Poll::Ready(Ok(n)) => {
                self.limit -= n as u64;
                Poll::Ready(Ok(n))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl async_std::io::Seek for ReaderWithLimit {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        let pos = match pos {
            SeekFrom::Start(pos) => SeekFrom::Start(self.range.start + pos),
            SeekFrom::End(offset) => SeekFrom::Start((self.range.end as i64 + offset) as u64),
            SeekFrom::Current(offset) => SeekFrom::Current(offset),
        };

        // println!("pos={:?}, range={:?}", pos, self.range);
        match Pin::new(self.reader.as_mut()).poll_seek(cx, pos) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(mut pos)) => {
                // println!("pos ret={}", pos);
                if pos < self.range.start {
                    let msg = format!("seek beyond the begin: {} < {}", pos, self.range.start);
                    let err = BuckyError::new(BuckyErrorCode::InvalidInput, msg);
                    return Poll::Ready(Err(err.into()));
                } else if pos > self.range.end {
                    pos = self.range.end;
                }

                self.limit = self.range.end - pos;
                Poll::Ready(Ok(pos - self.range.start))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncReadWithSeek for ReaderWithLimit {}

pub struct ChunkReaderWithHash {
    path: String,
    chunk_id: ChunkId,
    reader: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
    hash: sha2::Sha256,
}

impl ChunkReaderWithHash {
    pub fn new(
        path: String,
        chunk_id: ChunkId,
        reader: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
    ) -> Self {
        Self {
            path,
            chunk_id,
            reader,
            hash: sha2::Sha256::new(),
        }
    }
}

impl async_std::io::Read for ChunkReaderWithHash {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let ret = Pin::new(self.reader.as_mut()).poll_read(cx, buf);
        match ret {
            Poll::Ready(ret) => match ret {
                Ok(size) => {
                    if size > 0 {
                        self.hash.input(&buf[0..size]);
                        Poll::Ready(Ok(size))
                    } else {
                        let hash_value = self.hash.clone().result().into();
                        let actual_id = ChunkId::new(&hash_value, self.chunk_id.len() as u32);

                        if actual_id.eq(&self.chunk_id) {
                            debug!("read {} from file {:?}", self.chunk_id, self.path);
                            Poll::Ready(Ok(0))
                        } else {
                            let msg = format!(
                                "content in file {:?} not match chunk id: expect={}, got={}",
                                self.path, self.chunk_id, actual_id
                            );
                            error!("{}", msg);
                            let err = BuckyError::new(BuckyErrorCode::InvalidData, msg);
                            Poll::Ready(Err(err.into()))
                        }
                    }
                }
                Err(e) => Poll::Ready(Err(e)),
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

impl async_std::io::Seek for ChunkReaderWithHash {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        Pin::new(self.reader.as_mut()).poll_seek(cx, pos)
    }
}

impl AsyncReadWithSeek for ChunkReaderWithHash {}

#[cfg(test)]
mod tests {
    use async_std::io::prelude::*;
    use super::{ReaderWithLimit, ChunkReaderWithHash};
    use cyfs_base::*;
    use std::io::SeekFrom;
    use std::str::FromStr;

    async fn test_file() {
        let file = "C:\\cyfs\\data\\app\\cyfs-stack-test\\root\\test-chunk-in-bundle";
        let chunk_id = ChunkId::from_str("7C8WUcPdJGHvGxWou3HoABNe41Xhm9m3aEsSHfj1zeWG").unwrap();

        let buf = std::fs::read(file).unwrap();
        let real_id = ChunkId::calculate_sync(&buf).unwrap();
        assert_eq!(real_id, chunk_id);

        let reader = async_std::fs::File::open(file).await.unwrap();
        let mut reader =
                ChunkReaderWithHash::new("test1".to_owned(), chunk_id, Box::new(reader));

        let mut buf2 = vec![];
        reader.read_to_end(&mut buf2).await.unwrap();
    }

    async fn test1() {
        let buf: Vec<u8> = (0..3000).map(|_| rand::random::<u8>()).collect();
        let chunk_id = ChunkId::calculate(&buf).await.unwrap();

        {
            let buf_reader = Box::new(async_std::io::Cursor::new(buf.clone()));
            let mut reader =
                ChunkReaderWithHash::new("test1".to_owned(), chunk_id.clone(), buf_reader);

            let mut buf2 = vec![];
            reader.read_to_end(&mut buf2).await.unwrap();
            assert_eq!(buf, buf2);
        }

        let sub = &buf[1000..2000];
        let sub_chunk_id = ChunkId::calculate(&sub).await.unwrap();

        {
            let mut buf_reader = Box::new(async_std::io::Cursor::new(buf.clone()));
            buf_reader.seek(SeekFrom::Start(1000)).await.unwrap();

            let mut sub_reader = ReaderWithLimit::new(1000, buf_reader).await.unwrap();
            sub_reader.seek(SeekFrom::End(500)).await.unwrap();
            sub_reader.seek(SeekFrom::End(0)).await.unwrap();
            sub_reader.seek(SeekFrom::Start(0)).await.unwrap();

            let mut reader = ChunkReaderWithHash::new(
                "test2".to_owned(),
                sub_chunk_id.clone(),
                Box::new(sub_reader),
            );

            let mut buf2 = vec![];
            reader.read_to_end(&mut buf2).await.unwrap();
            assert_eq!(sub, buf2);
        }

        {
            let buf_reader = Box::new(async_std::io::Cursor::new(buf.clone()));

            let mut sub_reader = ReaderWithLimit::new(2000, buf_reader).await.unwrap();
            sub_reader.seek(SeekFrom::End(500)).await.unwrap();
            sub_reader.seek(SeekFrom::End(0)).await.unwrap();
            sub_reader.seek(SeekFrom::Start(1000)).await.unwrap();

            let mut reader = ChunkReaderWithHash::new(
                "test2".to_owned(),
                sub_chunk_id.clone(),
                Box::new(sub_reader),
            );

            let mut buf2 = vec![];
            reader.read_to_end(&mut buf2).await.unwrap();
            assert_eq!(sub, buf2);
        }

        {
            let buf_reader = Box::new(async_std::io::Cursor::new(buf.clone()));

            let mut sub_reader = ReaderWithLimit::new(2000, buf_reader).await.unwrap();
            let pos = sub_reader.seek(SeekFrom::End(-500)).await.unwrap();
            assert_eq!(pos, 1500);

            let mut buf2 = vec![];
            sub_reader.read_to_end(&mut buf2).await.unwrap();

            let sub = &buf[1500..2000];
            assert_eq!(sub, buf2);
        }

        {
            let mut buf_reader = Box::new(async_std::io::Cursor::new(buf.clone()));
            buf_reader.seek(SeekFrom::Start(500)).await.unwrap();

            let mut sub_reader = ReaderWithLimit::new(2000, buf_reader).await.unwrap();

            let pos = sub_reader.seek(SeekFrom::Start(0)).await.unwrap();
            assert_eq!(pos, 0);
            let pos = sub_reader.seek(SeekFrom::Current(1000)).await.unwrap();
            assert_eq!(pos, 1000);
            let pos = sub_reader.seek(SeekFrom::Current(1000)).await.unwrap();
            assert_eq!(pos, 2000);

            let pos = sub_reader.seek(SeekFrom::End(-500)).await.unwrap();
            assert_eq!(pos, 1500);

            let mut buf2 = vec![];
            sub_reader.read_to_end(&mut buf2).await.unwrap();

            let sub = &buf[2000..2500];
            assert_eq!(sub, buf2);
        }
        // sub_reader.seek(SeekFrom::Start(500)).await.unwrap();
    }

    #[test]
    fn test() {
        async_std::task::block_on(async move {
            test1().await;
            // test_file().await;
        });
    }
}
