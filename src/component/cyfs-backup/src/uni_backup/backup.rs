use super::chunk::*;
use super::object::*;
use crate::data::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct UniBackupManager {
    id: u64,

    noc: NamedObjectCacheRef,
    ndc: NamedDataCacheRef,

    loader: ObjectTraverserLoaderRef,
}

impl UniBackupManager {
    pub fn new(
        id: u64,
        noc: NamedObjectCacheRef,
        ndc: NamedDataCacheRef,
        loader: ObjectTraverserLoaderRef,
    ) -> Self {
        Self {
            id,
            noc,
            ndc,
            loader,
        }
    }

    pub async fn run(&self, data_writer: BackupDataWriterRef) -> BuckyResult<()> {
        info!("will backup uni objects: id={}", self.id);

        let backup = UniObjectBackup::new(self.noc.clone(), data_writer.clone(), self.loader.clone());
        backup.run().await?;

        info!("backup uni objects complete! id={}", self.id);

        info!("will backup uni chunks: id={}", self.id);

        let backup = UniChunkBackup::new(self.ndc.clone(), data_writer, self.loader.clone());
        backup.run().await?;

        info!("backup uni chunks complete! id={}", self.id);

        Ok(())
    }
}
