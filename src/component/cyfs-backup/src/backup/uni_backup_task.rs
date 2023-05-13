use super::backup_status::*;
use crate::archive::ObjectArchiveIndexHelper;
use crate::crypto::*;
use crate::key_data::*;
use crate::uni_backup::*;
use cyfs_backup_lib::*;
use cyfs_base::*;
use cyfs_bdt::ChunkReaderRef;
use cyfs_lib::*;

use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct UniBackupTask {
    id: String,
    noc: NamedObjectCacheRef,
    ndc: NamedDataCacheRef,
    chunk_reader: ChunkReaderRef,

    status_manager: BackupStatusManager,
}

impl UniBackupTask {
    pub fn new(
        id: String,
        noc: NamedObjectCacheRef,
        ndc: NamedDataCacheRef,
        chunk_reader: ChunkReaderRef,
    ) -> Self {
        Self {
            id,
            noc,
            ndc,
            chunk_reader,
            status_manager: BackupStatusManager::new(),
        }
    }

    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    pub fn status(&self) -> BackupStatus {
        self.status_manager.status()
    }

    pub async fn run(&self, params: UniBackupParams) -> BuckyResult<()> {
        let begin = std::time::Instant::now();
        let id = params.id.clone();

        let ret = self.run_inner(params).await;

        let r = match ret.as_ref() {
            Ok(_) => Ok(()),
            Err(e) => Err(e.clone()),
        };

        self.status_manager.on_complete(ret);
        self.status_manager.update_phase(BackupTaskPhase::Complete);

        let during = begin.elapsed();
        info!("run uni backup during: task={}, {:?}", id, during);

        r
    }

    pub async fn run_inner(&self, params: UniBackupParams) -> BuckyResult<BackupResult> {
        let loader = UniBackupObjectLoader::create(
            cyfs_util::get_cyfs_root_path_ref(),
            &params.isolate,
            self.chunk_reader.clone(),
        )
        .await?.into_reader();

        let device_file_name = if params.isolate.len() > 0 {
            format!("{}/device", params.isolate)
        } else {
            "device".to_owned()
        };

        let device = cyfs_util::LOCAL_DEVICE_MANAGER
            .load(&device_file_name)
            .map_err(|e| {
                let msg = format!(r#"invalid device.desc: {}, {}"#, device_file_name, e);
                error!("msg");
                BuckyError::new(e.code(), msg)
            })?;

        let device_id = device.device.desc().device_id();
        let owner = device.device.desc().owner().to_owned();

        info!("now will backup: device={}, owner={:?}", device_id, owner);

        self.status_manager.update_phase(BackupTaskPhase::Stat);

        self.run_stat(params.clone()).await?;

        self.status_manager.update_phase(BackupTaskPhase::Backup);
        let ret = self.run_backup(loader, device_id, owner, params).await;

        match ret {
            Ok((index, uni_meta)) => Ok(BackupResult {
                index,
                uni_meta: Some(uni_meta),
            }),
            Err(e) => Err(e),
        }
    }

    async fn run_stat(&self, params: UniBackupParams) -> BuckyResult<()> {
        let uni_stat = UniBackupStat::new(self.noc.clone(), self.ndc.clone());
        let uni_stat = uni_stat.stat().await?;

        let keydata = KeyDataManager::new_uni(&params.isolate, &params.key_data_filters)?;
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

    fn check_target_dir(dir: &Path) -> BuckyResult<()> {
        if dir.exists() {
            if dir.is_dir() {
                let mut read = dir.read_dir().map_err(|e| {
                    let msg = format!("read target dir failed! {}, {}", dir.display(), e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;

                if !read.next().is_none() {
                    let msg = format!(
                        "target dir is already exists and not empty! {}",
                        dir.display()
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }
            } else if dir.is_file() {
                let msg = format!(
                    "target dir is already exists and not valid dir! {}",
                    dir.display()
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        }

        Ok(())
    }

    pub fn backup_dir(params: &UniBackupParams) -> std::borrow::Cow<PathBuf> {
        match &params.target_file.dir {
            Some(dir) => std::borrow::Cow::Borrowed(dir),
            None => {
                let dir = if params.isolate.is_empty() {
                    cyfs_util::get_cyfs_root_path_ref().join(format!("data/backup/{}", params.id))
                } else {
                    cyfs_util::get_cyfs_root_path_ref()
                        .join(format!("data/backup/{}/{}", params.isolate, params.id))
                };

                std::borrow::Cow::Owned(dir)
            }
        }
    }

    async fn run_backup(
        &self,
        loader: ObjectTraverserLoaderRef,
        device_id: DeviceId,
        owner: Option<ObjectId>,
        params: UniBackupParams,
    ) -> BuckyResult<(ObjectArchiveIndex, ObjectArchiveMetaForUniBackup)> {
        let backup_dir = Self::backup_dir(&params);

        Self::check_target_dir(&backup_dir)?;

        info!("backup local dir is: {}", backup_dir.display());

        std::fs::create_dir_all(backup_dir.as_path()).map_err(|e| {
            let msg = format!(
                "create backup dir error: {}, err={}",
                backup_dir.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let crypto = match &params.password {
            Some(pw) => Some(AesKeyHelper::gen(pw.as_str(), &device_id)),
            None => None,
        };

        let uni_data_writer = UniBackupDataLocalFileWriter::new(
            params.id.clone(),
            backup_dir.to_path_buf(),
            params.target_file.data_folder.clone(),
            params.target_file.format,
            params.target_file.file_max_size,
            loader.clone(),
            crypto.clone(),
        )?;

        let data_writer = uni_data_writer.clone().into_writer();

        {
            let backup = UniBackupManager::new(
                params.id.clone(),
                self.noc.clone(),
                self.ndc.clone(),
                loader,
                self.status_manager.clone(),
            );

            backup.run(data_writer.clone()).await?;
        }

        let keydata_meta = {
            let keydata = KeyDataManager::new_uni(&params.isolate, &params.key_data_filters)?;
            let keydata_backup = KeyDataBackupManager::new(keydata, data_writer);

            keydata_backup.run().await.map_err(|e| {
                let msg = format!("backup key data failed! id={}, {}", params.id, e);
                error!("{}", e);
                BuckyError::new(e.code(), msg)
            })?
        };

        let (mut index, uni_meta) = uni_data_writer.finish().await?;

        let backup_meta = ObjectArchiveMetaForUniBackup::new(uni_meta, keydata_meta);
        let backup_meta_value = backup_meta.save()?;
        index.meta = Some(backup_meta_value);

        ObjectArchiveIndexHelper::init_device_id(&mut index, device_id, owner, crypto.as_ref());

        ObjectArchiveIndexHelper::save(&index, &backup_dir).await?;

        Ok((index, backup_meta))
    }
}
