use crate::data::*;
use crate::restore::*;
use cyfs_base::*;

pub struct UniRestoreManager {
    id: u64,
    backup_loader: BackupDataLoaderRef,
    restorer: ObjectRestorerRef,
}

impl UniRestoreManager {
    pub fn new(id: u64, backup_loader: BackupDataLoaderRef, restorer: ObjectRestorerRef) -> Self {
        Self {
            id,
            backup_loader,
            restorer,
        }
    }

    pub async fn run(&self) -> BuckyResult<()> {
        info!("will uni restore objects: id={}", self.id);
        
        self.backup_loader.reset_object().await;
        loop {
            let ret = self.backup_loader.next_object().await?;
            if ret.is_none() {
                break;
            }

            let (object_id, data) = ret.unwrap();
            self.restorer.restore_object(&object_id, data).await?;
        }

        info!("uni restore objects complete! id={}", self.id);

        info!("will uni restore chunks: id={}", self.id);

        self.backup_loader.reset_chunk().await;
        loop {
            let ret = self.backup_loader.next_chunk().await?;
            if ret.is_none() {
                break;
            }

            let (chunk_id, data) = ret.unwrap();
            self.restorer.restore_chunk(&chunk_id, data).await?;
        }

        info!("uni restore chunks complete! id={}", self.id);

        Ok(())
    }
}
