use crate::archive::*;
use cyfs_base::*;
use cyfs_lib::*;
use super::data::BackupDataManager;

use super::dec::DecStateBackup;

pub struct IsolateStateBackup {
    category: GlobalStateCategory,
    isolate_id: ObjectId,
    state_manager: GlobalStateRawProcessorRef,

    data_manager: BackupDataManager,
    loader: ObjectTraverserLoaderRef,
    meta_manager: GlobalStateMetaManagerRawProcessorRef,
}

impl IsolateStateBackup {
    pub fn new(
        category: GlobalStateCategory,
        isolate_id: ObjectId,
        data_manager: BackupDataManager,
        loader: ObjectTraverserLoaderRef,
        state_manager: GlobalStateRawProcessorRef,
        meta_manager: GlobalStateMetaManagerRawProcessorRef,
    ) -> Self {
        Self {
            category,
            isolate_id,
            state_manager,
            data_manager,
            loader,
            meta_manager,
        }
    }

    pub async fn backup_all_dec_list(&self) -> BuckyResult<ObjectArchiveIsolateMeta> {
        let info = self.state_manager.get_dec_root_info_list().await?;

        let mut isolate_meta =
            ObjectArchiveIsolateMeta::new(self.isolate_id, info.global_root, info.revision);
        for dec_info in info.dec_list {
            let meta = self
                .meta_manager
                .get_global_state_meta(&dec_info.dec_id, self.category, false)
                .await?;

            let dec_backup = DecStateBackup::new(
                self.isolate_id.clone(),
                dec_info.dec_id,
                dec_info.dec_root,
                self.data_manager.clone(),
                self.loader.clone(),
                meta,
            );
            let dec_meta = dec_backup.run().await?;
            isolate_meta.add_dec(dec_meta);
        }

        Ok(isolate_meta)
    }
}
