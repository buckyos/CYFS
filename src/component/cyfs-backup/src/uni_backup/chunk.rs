use crate::data::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct UniChunkBackup {
    ndc: NamedDataCacheRef,
    data_writer: BackupDataWriterRef,
    loader: ObjectTraverserLoaderRef,
}

impl UniChunkBackup {
    pub fn new(
        ndc: NamedDataCacheRef,
        data_writer: BackupDataWriterRef,
        loader: ObjectTraverserLoaderRef,
    ) -> Self {
        Self {
            ndc,
            data_writer,
            loader,
        }
    }

    pub async fn run(&self) -> BuckyResult<()> {
        let mut opt = SelectChunkOption::default();
        let filter = SelectChunkFilter {
            state: Some(ChunkState::Ready),
        };

        loop {
            let req = SelectChunkRequest {
                filter: filter.clone(),
                opt: opt.clone(),
            };

            let resp = self.ndc.select_chunk(&req).await?;
            let count = resp.list.len();

            for item in resp.list {
                self.on_chunk(item.chunk_id).await?;
            }

            if count < opt.page_size {
                break;
            }

            opt.page_index += 1;
        }

        Ok(())
    }

    async fn on_chunk(&self, chunk_id: ChunkId) -> BuckyResult<()> {
        self.data_writer.add_chunk(None, None, &chunk_id).await
    }
}
