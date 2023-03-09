use crate::archive::*;
use crate::object_pack::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::{AsyncReadWithSeek, AsyncReadWithSeekAdapter};

use async_std::sync::{Arc, Mutex as AsyncMutex};
use std::ops::DerefMut;
use std::path::PathBuf;

#[derive(Clone)]
pub struct ArchiveLocalFileWriter {
    archive: Arc<AsyncMutex<ObjectArchiveGenerator>>,
}

impl ArchiveLocalFileWriter {
    pub fn new(
        id: u64,
        root: PathBuf,
        format: ObjectPackFormat,
        archive_file_max_size: u64,
    ) -> BuckyResult<Self> {
        let data_dir = root.join("data");
        if !data_dir.is_dir() {
            std::fs::create_dir_all(&data_dir).map_err(|e| {
                let msg = format!(
                    "create backup data dir failed! {}, {}",
                    data_dir.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
        }

        let archive = ObjectArchiveGenerator::new(id, format, data_dir, archive_file_max_size);

        Ok(Self {
            archive: Arc::new(AsyncMutex::new(archive)),
        })
    }

    pub async fn add_object(
        &self,
        object_id: &ObjectId,
        object_raw: &[u8],
        meta: Option<&NamedObjectMetaData>,
    ) -> BuckyResult<u64> {
        let meta = meta.map(|item| item.into());

        let mut archive = self.archive.lock().await;
        let ret = archive.add_data_buf(object_id, object_raw, meta).await?;

        // For memory buffers, never fail
        Ok(ret.unwrap())
    }

    pub async fn add_chunk(
        &self,
        chunk_id: ChunkId,
        data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<BuckyResult<u64>> {
        let reader = AsyncReadWithSeekAdapter::new(data).into_reader();
        let mut archive = self.archive.lock().await;
        archive
            .add_data(chunk_id.as_object_id(), reader, meta)
            .await
    }

    pub async fn finish(&self) -> BuckyResult<ObjectArchiveIndex> {
        let archive = {
            let mut archive = self.archive.lock().await;
            let mut empty_archive = archive.clone_empty();
            std::mem::swap(archive.deref_mut(), &mut empty_archive);

            empty_archive
        };

        /*
        let archive = match Arc::try_unwrap(self.archive) {
            Ok(ret) => ret,
            Err(_) => unreachable!(),
        };

        let archive = archive.into_inner();
        */

        archive.finish().await
    }
}