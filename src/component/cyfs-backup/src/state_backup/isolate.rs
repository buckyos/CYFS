use crate::{meta::*, data::BackupDataWriterRef};
use cyfs_base::*;
use cyfs_lib::*;

use super::dec::DecStateBackup;

pub struct IsolateStateBackup {
    category: GlobalStateCategory,
    isolate_id: ObjectId,
    state_manager: GlobalStateRawProcessorRef,

    data_writer: BackupDataWriterRef,
    loader: ObjectTraverserLoaderRef,
    meta_manager: GlobalStateMetaManagerRawProcessorRef,
}

impl IsolateStateBackup {
    pub fn new(
        category: GlobalStateCategory,
        isolate_id: ObjectId,
        data_writer: BackupDataWriterRef,
        loader: ObjectTraverserLoaderRef,
        state_manager: GlobalStateRawProcessorRef,
        meta_manager: GlobalStateMetaManagerRawProcessorRef,
    ) -> Self {
        Self {
            category,
            isolate_id,
            state_manager,
            data_writer,
            loader,
            meta_manager,
        }
    }

    pub async fn backup_all_dec_list(&self) -> BuckyResult<ObjectArchiveIsolateMeta> {
        self.backup_dec_list_with_filter(None).await
    }

    pub async fn backup_dec_list(
        &self,
        dec_list: &[ObjectId],
    ) -> BuckyResult<ObjectArchiveIsolateMeta> {
        self.backup_dec_list_with_filter(Some(dec_list)).await
    }

    async fn backup_dec_list_with_filter(
        &self,
        dec_list: Option<&[ObjectId]>,
    ) -> BuckyResult<ObjectArchiveIsolateMeta> {
        let info = self.state_manager.get_dec_root_info_list().await?;

        let mut isolate_meta =
            ObjectArchiveIsolateMeta::new(self.isolate_id, info.global_root, info.revision);
        for dec_info in info.dec_list {
            if let Some(dec_list) = &dec_list {
                if !dec_list.contains(&dec_info.dec_id) {
                    continue;
                }
            }

            info!(
                "will backup dec data: ioslate={}, {:?}",
                self.isolate_id, dec_info
            );

            let meta = self
                .meta_manager
                .get_global_state_meta(&dec_info.dec_id, self.category, false)
                .await?;

            let dec_backup = DecStateBackup::new(
                self.isolate_id.clone(),
                dec_info.dec_id.clone(),
                dec_info.dec_root.clone(),
                self.data_writer.clone(),
                self.loader.clone(),
                meta,
            );
            let dec_meta = dec_backup.run().await?;

            info!(
                "backup dec data complete: isolate={}, {:?}, meta={:?}",
                self.isolate_id, dec_info, dec_meta
            );

            isolate_meta.add_dec(dec_meta);
        }

        Ok(isolate_meta)
    }
}
