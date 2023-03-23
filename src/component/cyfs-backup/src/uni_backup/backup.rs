use super::chunk::*;
use super::object::*;
use crate::backup::BackupStatusManager;
use crate::data::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct UniBackupManager {
    id: String,

    noc: NamedObjectCacheRef,
    ndc: NamedDataCacheRef,

    loader: ObjectTraverserLoaderRef,
    status_manager: BackupStatusManager,
}

impl UniBackupManager {
    pub fn new(
        id: String,
        noc: NamedObjectCacheRef,
        ndc: NamedDataCacheRef,
        loader: ObjectTraverserLoaderRef,
        status_manager: BackupStatusManager,
    ) -> Self {
        Self {
            id,
            noc,
            ndc,
            loader,
            status_manager,
        }
    }

    pub async fn run(&self, data_writer: BackupDataWriterRef) -> BuckyResult<()> {
        info!("will uni backup objects: id={}", self.id);

        let backup = UniObjectBackup::new(
            self.noc.clone(),
            data_writer.clone(),
            self.loader.clone(),
            self.status_manager.clone(),
        );
        backup.run().await?;

        info!("uni backup objects complete! id={}", self.id);

        info!("will uni backup chunks: id={}", self.id);

        let backup = UniChunkBackup::new(
            self.ndc.clone(),
            data_writer,
            self.loader.clone(),
            self.status_manager.clone(),
        );
        backup.run().await?;

        info!("uni backup chunks complete! id={}", self.id);

        Ok(())
    }
}
