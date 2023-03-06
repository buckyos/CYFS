use super::isolate::*;
use crate::archive::*;
use crate::data::*;
use crate::object_pack::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::path::PathBuf;

pub struct GlobalStateBackup {
    id: u64,
    category: GlobalStateCategory,
    format: ObjectPackFormat,

    root: PathBuf,
    default_isolate: ObjectId,

    state_manager: GlobalStateManagerRawProcessorRef,
    loader: ObjectTraverserLoaderRef,
    meta_manager: GlobalStateMetaManagerRawProcessorRef,
}

#[derive(Debug)]
pub struct GlobalStateIsolateBackupFilter {
    pub isolate_id: ObjectId,
    pub dec_list: Vec<ObjectId>,
}

#[derive(Debug)]
pub struct GlobalStateBackupFilter {
    pub isolate_list: Vec<GlobalStateIsolateBackupFilter>,
}

#[derive(Debug)]
pub struct GlobalStateBackupParams {
    pub filter: GlobalStateBackupFilter,
}

impl GlobalStateBackup {
    pub fn new(
        id: u64,
        root: PathBuf,
        default_isolate: ObjectId,
        state_manager: GlobalStateManagerRawProcessorRef,
        loader: ObjectTraverserLoaderRef,
        meta_manager: GlobalStateMetaManagerRawProcessorRef,
    ) -> Self {
        Self {
            id,
            category: GlobalStateCategory::RootState,
            format: ObjectPackFormat::Zip,
            default_isolate,
            root,
            state_manager,
            loader,
            meta_manager,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub async fn backup(&self, params: GlobalStateBackupParams) -> BuckyResult<ObjectArchiveMeta> {
        self.backup_with_filter(params.filter).await
    }

    async fn backup_with_filter(
        &self,
        filters: GlobalStateBackupFilter,
    ) -> BuckyResult<ObjectArchiveMeta> {
        let backup_dir = self.root.join(format!("{}", self.id));

        let data_writer = BackupDataLocalFileWriter::new(
            self.id,
            self.default_isolate.clone(),
            backup_dir,
            self.format,
            1024 * 1024 * 128,
        )?;

        self.backup_impl(data_writer.clone().into_writer(), filters).await?;

        let meta = data_writer.finish().await.map_err(|e| {
            let msg = format!("backup global state but finish failed! {}", e);
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        Ok(meta)
    }

    pub async fn stat(&self, params: GlobalStateBackupParams) -> BuckyResult<ObjectArchiveStatMeta> {
        let data_writer = BackupDataStatWriter::new(self.id);

        self.backup_impl(data_writer.clone().into_writer(), params.filter).await?;

        let meta = data_writer.finish().await.map_err(|e| {
            let msg = format!("stat global state but finish failed! {}", e);
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        Ok(meta)
    }

    async fn backup_impl(
        &self,
        data_writer: BackupDataWriterRef,
        filters: GlobalStateBackupFilter,
    ) -> BuckyResult<()> {
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
                data_writer.clone(),
                self.loader.clone(),
                isolate_state_manager,
                self.meta_manager.clone(),
            );

            let isolate_meta = backup
                .backup_dec_list(isolate_filter.dec_list.as_ref())
                .await?;
            data_writer.add_isolate_meta(isolate_meta).await;
        }

        Ok(())
    }
}
