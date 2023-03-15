use crate::backup::*;
use crate::data::*;
use crate::meta::*;
use crate::restore::*;
use cyfs_base::*;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct UniRestoreDataFilter {
    chunks: Arc<Mutex<HashSet<ChunkId>>>,
}

impl UniRestoreDataFilter {
    pub fn new() -> Self {
        Self {
            chunks: Arc::new(Mutex::new(HashSet::with_capacity(128))),
        }
    }

    pub fn append_key_data_chunks(&self, key_data_meta: &Vec<KeyDataMeta>) {
        let mut chunks = self.chunks.lock().unwrap();
        for item in key_data_meta.iter() {
            chunks.insert(item.chunk_id.clone());
        }
    }

    pub fn filter_chunk(&self, chunk_id: &ChunkId) -> bool {
        self.chunks.lock().unwrap().contains(chunk_id)
    }
}

pub struct UniRestoreManager {
    id: String,
    backup_loader: BackupDataLoaderRef,
    restorer: ObjectRestorerRef,
    filter: UniRestoreDataFilter,
    status_manager: RestoreStatusManager,
}

impl UniRestoreManager {
    pub fn new(
        id: String,
        backup_loader: BackupDataLoaderRef,
        restorer: ObjectRestorerRef,
        filter: UniRestoreDataFilter,
        status_manager: RestoreStatusManager,
    ) -> Self {
        Self {
            id,
            backup_loader,
            restorer,
            filter,
            status_manager,
        }
    }

    pub async fn run(&self) -> BuckyResult<()> {
        info!("will uni restore objects: id={}", self.id);

        self.status_manager.update_phase(RestoreTaskPhase::RestoreObject);

        self.backup_loader.reset_object().await;
        loop {
            let ret = self.backup_loader.next_object().await?;
            if ret.is_none() {
                break;
            }

            let (object_id, data) = ret.unwrap();
            self.restorer.restore_object(&object_id, data).await?;

            self.status_manager.on_object();
        }

        info!("uni restore objects complete! id={}", self.id);

        self.status_manager.update_phase(RestoreTaskPhase::RestoreChunk);

        info!("will uni restore chunks: id={}", self.id);

        self.backup_loader.reset_chunk().await;
        loop {
            let ret = self.backup_loader.next_chunk().await?;
            if ret.is_none() {
                break;
            }

        
            let (chunk_id, data) = ret.unwrap();
            if self.filter.filter_chunk(&chunk_id) {
                warn!("will ignore chunk on filter: {}", chunk_id);
                self.status_manager.on_chunk();
                continue;
            }

            self.restorer.restore_chunk(&chunk_id, data).await?;
            self.status_manager.on_chunk();
        }

        info!("uni restore chunks complete! id={}", self.id);

        Ok(())
    }
}
