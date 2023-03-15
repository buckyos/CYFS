use super::roots::*;
use super::state::GlobalStateBackup;
use super::writer::StateBackupDataLocalFileWriter;
use crate::archive::*;
use crate::data::*;
use crate::meta::*;
use crate::object_pack::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::path::PathBuf;

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

pub struct StateBackupManager {
    id: String,
    root: PathBuf,
    format: ObjectPackFormat,
    state_default_isolate: ObjectId,

    noc: NamedObjectCacheRef,
    state_manager: GlobalStateManagerRawProcessorRef,
    loader: ObjectTraverserLoaderRef,
    meta_manager: GlobalStateMetaManagerRawProcessorRef,
}

impl StateBackupManager {
    pub fn new(
        id: String,
        root: PathBuf,
        state_default_isolate: ObjectId,
        noc: NamedObjectCacheRef,
        state_manager: GlobalStateManagerRawProcessorRef,
        loader: ObjectTraverserLoaderRef,
        meta_manager: GlobalStateMetaManagerRawProcessorRef,
    ) -> Self {
        Self {
            id,
            format: ObjectPackFormat::Zip,
            state_default_isolate,
            root,
            noc,
            state_manager,
            loader,
            meta_manager,
        }
    }

    pub async fn backup(
        &self,
        params: GlobalStateBackupParams,
    ) -> BuckyResult<(ObjectArchiveIndex, ObjectArchiveStateMeta)> {
        let backup_dir = self.root.join(format!("{}", self.id));

        let data_writer = StateBackupDataLocalFileWriter::new(
            self.id.clone(),
            self.state_default_isolate.clone(),
            backup_dir,
            self.format,
            1024 * 1024 * 128,
            self.loader.clone(),
        )?;

        let writer = data_writer.clone().into_writer();

        let root_meta = self.run(params, writer).await?;

        let (index, mut meta) = data_writer.finish().await.map_err(|e| {
            let msg = format!("backup but finish failed! {}", e);
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        meta.roots = root_meta;

        Ok((index, meta))
    }

    pub async fn stat(
        &self,
        params: GlobalStateBackupParams,
    ) -> BuckyResult<ObjectArchiveStatMeta> {
        let data_writer = BackupDataStatWriter::new(self.id.clone());
        let writer = data_writer.clone().into_writer();

        let root_meta = self.run(params, writer).await?;

        let mut meta = data_writer.finish().await.map_err(|e| {
            let msg = format!("stat global state but finish failed! {}", e);
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        meta.roots = root_meta;

        Ok(meta)
    }

    async fn run(
        &self,
        params: GlobalStateBackupParams,
        data_writer: BackupDataWriterRef,
    ) -> BuckyResult<ObjectArchiveDataSeriesMeta> {
        info!("will backup root state: id={}", self.id);

        let state_backup = GlobalStateBackup::new(
            GlobalStateCategory::RootState,
            data_writer.clone(),
            self.state_manager.clone(),
            self.loader.clone(),
            self.meta_manager.clone(),
        );
        state_backup.run(params).await?;

        info!("backup root state complete! id={}", self.id);

        info!("will backup all root objects: id={}", self.id);

        let roots_backup =
            RootObjectBackup::new(self.noc.clone(), data_writer, self.loader.clone());
        let roots_meta = roots_backup.run().await?;

        info!("backup all root objects complete! id={}", self.id);

        Ok(roots_meta)
    }
}
