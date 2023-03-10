use super::uni_backup_task::*;
use cyfs_base::*;
use cyfs_bdt::ChunkReaderRef;
use cyfs_lib::*;

use std::sync::Arc;


pub struct BackupManager {
    isolate: String,
    state_default_isolate: ObjectId,
    noc: NamedObjectCacheRef,
    ndc: NamedDataCacheRef,
    state_manager: GlobalStateManagerRawProcessorRef,
    meta_manager: GlobalStateMetaManagerRawProcessorRef,
    loader: ObjectTraverserLoaderRef,
}

impl BackupManager {
    pub fn new(
        isolate: &str,
        state_default_isolate: ObjectId,
        noc: NamedObjectCacheRef,
        ndc: NamedDataCacheRef,
        state_manager: GlobalStateManagerRawProcessorRef,
        meta_manager: GlobalStateMetaManagerRawProcessorRef,
        chunk_reader: ChunkReaderRef,
    ) -> Self {
        let loader = ObjectTraverserLocalLoader::new(noc.clone(), chunk_reader).into_reader();
        Self {
            isolate: isolate.to_owned(),
            state_default_isolate,
            noc,
            ndc,
            state_manager,
            meta_manager,
            loader,
        }
    }

    pub async fn run_uni_backup(&self, params: UniBackupParams) -> BuckyResult<()> {
        let task = UniBackupTask::new(
            &self.isolate,
            self.noc.clone(),
            self.ndc.clone(),
            self.loader.clone(),
        );

        task.run(params).await
    }
}

pub type BackupManagerRef = Arc<BackupManager>;
