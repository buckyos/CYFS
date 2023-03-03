use crate::archive::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::{AsyncReadWithSeek, AsyncReadWithSeekAdapter};

use async_std::sync::{Arc, Mutex as AsyncMutex};

#[derive(Clone)]
pub struct BackupDataManager {
    archive: Arc<AsyncMutex<ObjectArchiveGenerator>>,
}

impl BackupDataManager {
    pub async fn add_object(
        &self,
        object_id: &ObjectId,
        object_raw: &[u8],
        meta: Option<&NamedObjectMetaData>,
    ) -> BuckyResult<()> {
        let meta = meta.map(|item| {
            item.into()
        });

        let mut archive = self.archive.lock().await;
        archive.add_data_buf(object_id, object_raw, meta).await?;

        Ok(())
    }

    pub async fn add_data(
        &self,
        object_id: ObjectId,
        data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<()> {
        //let meta = meta.map(|item| {
        //    item.into()
        //});

        let reader = AsyncReadWithSeekAdapter::new(data).into_reader();
        let mut archive = self.archive.lock().await;
        archive.add_data(&object_id, reader, meta).await?;

        Ok(())
    }
}
