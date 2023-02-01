
use cyfs_base::*;
use cyfs_bdt::{ChunkListTaskReader, ChunkTaskReader};
use cyfs_chunk_lib::Chunk;

use async_std::io::ReadExt;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ChunkWriter: Send + Sync {
    // 写入一组chunk到文件
    async fn write(&self, chunk: &ChunkId, chunk: Box<dyn Chunk>) -> BuckyResult<()>;
    async fn finish(&self) -> BuckyResult<()>;
    async fn err(&self, e: &BuckyError) -> BuckyResult<()>;
}

pub type ChunkWriterRef  = Arc<Box<dyn ChunkWriter>>;

pub struct ChunkListReaderAdapter {
    writer: ChunkWriterRef,
    reader: Box<dyn async_std::io::Read + Unpin + Sync + Send + 'static>,
    chunk_list: ChunkList,
}

impl ChunkListReaderAdapter {
    pub fn new_file(
        writer: ChunkWriterRef,
        reader: ChunkListTaskReader,
        file: &File,
    ) -> Self {
        Self {
            writer,
            reader: Box::new(reader),
            chunk_list: file.body().as_ref().unwrap().content().chunk_list().clone(),
        }
    }

    pub fn new_chunk(
        writer: ChunkWriterRef,
        reader: ChunkTaskReader,
        chunk_id: &ChunkId,
    ) -> Self {
        Self {
            writer,
            reader: Box::new(reader),
            chunk_list: ChunkList::ChunkInList(vec![chunk_id.to_owned()]),
        }
    }

    pub fn async_run(self) {
        async_std::task::spawn(async move {
            let _ = self.run().await;
        });
    }

    pub async fn run(mut self) -> BuckyResult<()> {
        let chunk_list = self.chunk_list.inner_chunk_list().unwrap();
        let mut buffer: Vec<u8> = vec![0; 1024 * 1024 * 4];

        for chunk_id in chunk_list {
            let len = chunk_id.len();
            if buffer.len() < len {
                buffer.resize(len, 0);
            }

            let buf = &mut buffer[..len];
            if let Err(e) = self.reader.read_exact(buf).await {
                let msg = format!("read chunk from reader failed! chunk={}, {}", chunk_id, e);
                error!("{}", msg);
                let err = BuckyError::new(BuckyErrorCode::IoError, msg);
                self.writer.err(&err).await?;

                return Err(err);
            }

            // TODO use stream instead buffer to opt for the memory usage
            let ref_chunk = cyfs_chunk_lib::MemRefChunk::from(unsafe {
                std::mem::transmute::<_, &'static [u8]>(buf)
            });
            let content = Box::new(ref_chunk) as Box<dyn Chunk>;
            self.writer.write(chunk_id, content).await?;
        }

        self.writer.finish().await?;

        Ok(())
    }
}
