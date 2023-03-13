use super::status::*;
use crate::archive::ObjectArchiveIndex;
use crate::key_data::*;
use crate::meta::*;
use crate::object_pack::*;
use crate::uni_backup::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LocalFileBackupParam {
    // Backup file storage directory
    pub dir: Option<PathBuf>,

    pub format: ObjectPackFormat,

    pub file_max_size: u64,
}

impl Default for LocalFileBackupParam {
    fn default() -> Self {
        Self {
            dir: None,
            format: ObjectPackFormat::Zip,
            file_max_size: 1024 * 1024 * 512,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UniBackupParams {
    pub id: u64,

    pub file: LocalFileBackupParam,
}

#[derive(Clone)]
pub struct UniBackupTask {
    id: u64,
    isolate: String,
    noc: NamedObjectCacheRef,
    ndc: NamedDataCacheRef,
    loader: ObjectTraverserLoaderRef,

    status_manager: BackupStatusManager,
}

impl UniBackupTask {
    pub fn new(
        id: u64,
        isolate: &str,
        noc: NamedObjectCacheRef,
        ndc: NamedDataCacheRef,
        loader: ObjectTraverserLoaderRef,
    ) -> Self {
        Self {
            id,
            isolate: isolate.to_owned(),
            noc,
            ndc,
            loader,
            status_manager: BackupStatusManager::new()
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
    
    pub fn status(&self) -> BackupStatus {
        self.status_manager.status()
    }

    pub async fn run(&self, params: UniBackupParams) -> BuckyResult<()> {
        self.status_manager.update_phase(BackupTaskPhase::Stat);

        self.run_stat(params.clone()).await?;

        self.status_manager.update_phase(BackupTaskPhase::Backup);
        let ret = self.run_backup(params).await;

        let ret = match ret {
            Ok((index, uni_meta)) => Ok(BackupResult {
                index,
                uni_meta: Some(uni_meta),
            }),
            Err(e) => Err(e),
        };

        self.status_manager.on_complete(ret);

        self.status_manager.update_phase(BackupTaskPhase::Complete);

        Ok(())
    }

    async fn run_stat(&self, _params: UniBackupParams) -> BuckyResult<()> {
        let uni_stat = UniBackupStat::new(self.noc.clone(), self.ndc.clone());
        let uni_stat = uni_stat.stat().await?;

        let keydata = KeyDataManager::new_uni(&self.isolate);
        let keydata_stat = KeyDataBackupStat::new(keydata);
        let keydata_stat = keydata_stat.stat();

        let stat = BackupStatInfo {
            objects: uni_stat.objects,
            chunks: uni_stat.chunks,
            files: keydata_stat,
        };

        self.status_manager.init_stat(stat);

        Ok(())
    }

    async fn run_backup(
        &self,
        params: UniBackupParams,
    ) -> BuckyResult<(ObjectArchiveIndex, ObjectArchiveMetaForUniBackup)> {
        
        let backup_dir = match params.file.dir {
            Some(dir) => dir,
            None => cyfs_util::get_cyfs_root_path_ref().join(format!("data/backup/{}", params.id)),
        };

        info!("backup local dir is: {}", backup_dir.display());

        std::fs::create_dir_all(&backup_dir).map_err(|e| {
            let msg = format!(
                "create backup dir error: {}, err={}",
                backup_dir.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let uni_data_writer = UniBackupDataLocalFileWriter::new(
            params.id,
            backup_dir.clone(),
            params.file.format,
            params.file.file_max_size,
            self.loader.clone(),
        )?;

        let data_writer = uni_data_writer.clone().into_writer();

        {
            let backup = UniBackupManager::new(
                params.id,
                self.noc.clone(),
                self.ndc.clone(),
                self.loader.clone(),
                self.status_manager.clone(),
            );

            backup.run(data_writer.clone()).await?;
        }

        let keydata_meta = {
            let keydata = KeyDataManager::new_uni(&self.isolate);
            let keydata_backup = KeyDataBackupManager::new(keydata, data_writer);

            keydata_backup.run().await.map_err(|e| {
                let msg = format!("backup key data failed! id={}, {}", params.id, e);
                error!("{}", e);
                BuckyError::new(e.code(), msg)
            })?
        };

        let (index, uni_meta) = uni_data_writer.finish().await?;

        let backup_meta = ObjectArchiveMetaForUniBackup::new(params.id, uni_meta, keydata_meta);

        index.save(&backup_dir).await?;
        backup_meta.save(&backup_dir).await?;

        Ok((index, backup_meta))
    }
}
