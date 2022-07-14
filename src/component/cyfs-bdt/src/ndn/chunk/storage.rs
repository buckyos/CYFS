use std::{
    ops::Range, 
    collections::LinkedList
};
use async_trait::async_trait;
use async_std::{ 
    sync::Arc, 
    io::{prelude::*, Cursor}, 
    pin::Pin, 
    task::{Context, Poll}
};
use cyfs_base::*;


#[async_trait]
pub trait ChunkWriter: 'static + std::fmt::Display + Send + Sync {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter>;
    // 写入一组chunk到文件
    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()>;
    async fn finish(&self) -> BuckyResult<()>;
    async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()>;
}


#[async_trait]
pub trait ChunkWriterExt: 'static + std::fmt::Display + Send + Sync {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt>;
    // 写入一组chunk到文件
    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>, range: Option<Range<u64>>) -> BuckyResult<()>;
    async fn finish(&self) -> BuckyResult<()>;
    async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()>;
}

pub struct ChunkWriterExtWrapper {
    writer: Box<dyn ChunkWriter>
}

impl ChunkWriterExtWrapper {
    pub fn new(writer: Box<dyn ChunkWriter>) -> Self {
        Self {
            writer
        }
    }
}


impl std::fmt::Display for ChunkWriterExtWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.writer)
    }
}


#[async_trait]
impl ChunkWriterExt for ChunkWriterExtWrapper {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt> {
        Box::new(Self {
            writer: self.writer.clone_as_writer()
        })
    }

    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>, range: Option<Range<u64>>) -> BuckyResult<()> {
        if range.is_some() {
            let range = range.clone().unwrap();
            if range.start != 0 
                || range.end != chunk.len() as u64 {
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, "not suppor range"));
            }
        }
        self.writer.write(chunk, content).await
    }

    async fn finish(&self) -> BuckyResult<()> {
        self.writer.finish().await
    }

    async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()> {
        self.writer.err(e).await
    }


}




#[async_trait]
pub trait ChunkReader: 'static + Send + Sync {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader>;
    async fn exists(&self, chunk: &ChunkId) -> bool;
    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Arc<Vec<u8>>>;
    async fn read(&self, chunk: &ChunkId) -> BuckyResult<Box<dyn Read + Unpin + Send + Sync>> {
        struct ArcWrap(Arc<Vec<u8>>);
        impl AsRef<[u8]> for ArcWrap {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }
        
        let data = ArcWrap(self.get(chunk).await?);

        Ok(Box::new(Cursor::new(data)))
    } 

    async fn read_ext(&self, chunk: &ChunkId, ranges: Vec<Range<u64>>) -> BuckyResult<Box<dyn Read + Unpin + Send + Sync>> {
        struct ChainReader<'a> {
            pieces: LinkedList<Box<dyn 'a + Unpin + Read + Send + Sync>>, 
        }

        impl<'a> ChainReader<'a> {
            pub fn new(pieces: LinkedList<Box<dyn 'a + Unpin + Read + Send + Sync>>) -> Self {
                Self { pieces }
            }
        }

        impl<'a> Read for ChainReader<'a> {
            fn poll_read(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut [u8],
            ) -> Poll<std::io::Result<usize>> { 
                let pined = self.get_mut();
                let pieces = &mut pined.pieces;
                while let Some(mut front) = pieces.pop_front() {
                    match Pin::new(&mut front).poll_read(cx, buf) {
                        Poll::Ready(r) => {
                            match r {
                                Ok(r) => {
                                    if r > 0 {
                                        pieces.push_front(front);
                                        return Poll::Ready(Ok(r));
                                    } else {
                                        continue;
                                    }
                                }, 
                                Err(err) => {
                                    pieces.push_front(front);
                                    return Poll::Ready(Err(err));
                                }
                            }
                        }, 
                        Poll::Pending => {
                            pieces.push_front(front);
                            return Poll::Pending;
                        }
                    }
                } 

                Poll::Ready(Ok(0))
            }
        }


        struct ArcWrap(Arc<Vec<u8>>, Range<usize>);
        impl AsRef<[u8]> for ArcWrap {
            fn as_ref(&self) -> &[u8] {
                &self.0.as_ref()[self.1.clone()]
            }
        }

        if ranges.len() == 0 {
            Ok(Box::new(Cursor::new(vec![])))
        } else if ranges.len() == 1 {
            Ok(Box::new(Cursor::new(ArcWrap(self.get(chunk).await?, ranges[0].start as usize..ranges[0].end as usize))))
        } else {
            let mut readers = LinkedList::<Box<dyn Unpin + Read + Send + Sync>>::new();
            let content = self.get(chunk).await?;
            for range in ranges.into_iter() {
                readers.push_back(Box::new(Cursor::new(ArcWrap(content.clone(), range.start as usize..range.end as usize))));
            }
            Ok(Box::new(ChainReader::new(readers)))
        }
    }
}


