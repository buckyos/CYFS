use crate::key_data::*;
use crate::meta::ObjectArchiveMetaForUniBackup;
use crate::object_pack::*;
use crate::uni_backup::*;
use cyfs_base::*;
use cyfs_bdt::ChunkReaderRef;
use cyfs_lib::*;

use std::path::PathBuf;

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

pub struct UniBackupParams {
    id: u64,

    file: LocalFileBackupParam,
}

pub struct BackupManager {
    default_isolate: ObjectId,
    noc: NamedObjectCacheRef,
    ndc: NamedDataCacheRef,
    state_manager: GlobalStateManagerRawProcessorRef,
    meta_manager: GlobalStateMetaManagerRawProcessorRef,
    loader: ObjectTraverserLoaderRef,
}

impl BackupManager {
    pub fn new(
        default_isolate: ObjectId,
        noc: NamedObjectCacheRef,
        ndc: NamedDataCacheRef,
        state_manager: GlobalStateManagerRawProcessorRef,
        meta_manager: GlobalStateMetaManagerRawProcessorRef,
        chunk_reader: ChunkReaderRef,
    ) -> Self {
        let loader = ObjectTraverserLocalLoader::new(noc.clone(), chunk_reader).into_reader();
        Self {
            default_isolate,
            noc,
            ndc,
            state_manager,
            meta_manager,
            loader,
        }
    }

    pub async fn uni_backup(&self, params: UniBackupParams) -> BuckyResult<()> {
        let backup_dir = match params.file.dir {
            Some(dir) => dir,
            None => cyfs_util::get_app_data_dir("backup").join(format!("{}", params.id)),
        };

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
            );

            backup.run(data_writer.clone()).await?;
        }

        let keydata_meta = {
            let keydata = KeyDataManager::new_uni();
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
        
        Ok(())
    }
}