use super::backup::{GlobalStateBackupFilter, GlobalStateBackupParams};
use super::isolate::*;
use crate::data::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct GlobalStateBackup {
    category: GlobalStateCategory,

    data_writer: BackupDataWriterRef,
    state_manager: GlobalStateManagerRawProcessorRef,
    loader: ObjectTraverserLoaderRef,
    meta_manager: GlobalStateMetaManagerRawProcessorRef,
}

impl GlobalStateBackup {
    pub fn new(
        category: GlobalStateCategory,
        data_writer: BackupDataWriterRef,
        state_manager: GlobalStateManagerRawProcessorRef,
        loader: ObjectTraverserLoaderRef,
        meta_manager: GlobalStateMetaManagerRawProcessorRef,
    ) -> Self {
        Self {
            category,
            data_writer,
            state_manager,
            loader,
            meta_manager,
        }
    }

    pub async fn run(&self, params: GlobalStateBackupParams) -> BuckyResult<()> {
        self.backup_impl(params.filter).await
    }

    async fn backup_impl(&self, filters: GlobalStateBackupFilter) -> BuckyResult<()> {
        for isolate_filter in filters.isolate_list {
            if isolate_filter.dec_list.is_empty() {
                warn!(
                    "isolate's dec_list is empty! isolate={}, category={}",
                    isolate_filter.isolate_id, self.category
                );
                continue;
            }

            let ret = self
                .state_manager
                .get_global_state(self.category, &isolate_filter.isolate_id)
                .await;
            if ret.is_none() {
                warn!(
                    "isolate's state not exists! isolate={}, category={}",
                    isolate_filter.isolate_id, self.category
                );
                continue;
            }

            let isolate_state_manager = ret.unwrap();
            let backup = IsolateStateBackup::new(
                self.category,
                isolate_filter.isolate_id,
                self.data_writer.clone(),
                self.loader.clone(),
                isolate_state_manager,
                self.meta_manager.clone(),
            );

            let isolate_meta = backup
                .backup_dec_list(isolate_filter.dec_list.as_ref())
                .await?;
            self.data_writer.add_isolate_meta(isolate_meta).await;
        }

        Ok(())
    }
}
