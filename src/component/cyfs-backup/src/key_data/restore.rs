use crate::archive::*;
use crate::data::*;
use crate::meta::*;
use crate::restore::ObjectRestorerRef;
use cyfs_base::*;
use crate::backup::*;

use std::io::Read;
use std::path::PathBuf;
use zip::ZipArchive;

pub struct KeyDataRestoreManager {
    list: Vec<KeyDataMeta>,
    data_loader: BackupDataLoaderRef,
    restorer: ObjectRestorerRef,
    status_manager: RestoreStatusManager,
}

impl KeyDataRestoreManager {
    pub fn new(
        keydata: Vec<KeyDataMeta>,
        data_loader: BackupDataLoaderRef,
        restorer: ObjectRestorerRef,
        status_manager: RestoreStatusManager,
    ) -> Self {
        Self {
            list: keydata,
            data_loader,
            restorer,
            status_manager,
        }
    }

    pub async fn run(&self) -> BuckyResult<()> {
        // The recovery operation is performed in reverse order to ensure that {cyfs}/etc/desc is restored at the end
        for item in self.list.iter().rev() {
            info!("will restore key data: {:?}", item);

            self.restore_data(item).await?;
            self.status_manager.on_file();
        }

        info!("restore all key data complete!");

        Ok(())
    }

    async fn restore_data(&self, meta: &KeyDataMeta) -> BuckyResult<()> {
        let ret = self
            .data_loader
            .get_chunk(&meta.chunk_id)
            .await
            .map_err(|e| {
                let msg = format!(
                    "restore key data but load chunk failed! file={}, chunk={}, {}",
                    meta.local_path, meta.chunk_id, e
                );
                error!("{}", msg);
                BuckyError::new(e.code(), msg)
            })?;

        if ret.is_none() {
            let msg = format!(
                "restore key data but chunk not found! file={}, chunk={}",
                meta.local_path, meta.chunk_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let data = ret.unwrap();
        match meta.data_type {
            KeyDataType::File => {
                let path = PathBuf::from(&meta.local_path);
                self.restorer
                    .restore_file(&path, data.data)
                    .await
                    .map_err(|e| {
                        let msg = format!(
                            "restore key data file failed! file={}, chunk={}, {}",
                            meta.local_path, meta.chunk_id, e
                        );
                        error!("{}", msg);
                        BuckyError::new(e.code(), msg)
                    })?;
            }
            KeyDataType::Dir => {
                let buf = data.data.into_buffer().await?;
                let mut s = std::io::Cursor::new(buf);

                let mut ar = ZipArchive::new(&mut s).map_err(|e| {
                    let msg = format!(
                        "load chunk as zip dir failed! file={}, chunk={}, {}",
                        meta.local_path, meta.chunk_id, e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

                for i in 0..ar.len() {
                    let mut content;
                    let file_path;
                    {
                        let mut file = ar.by_index(i)?;
                        if file.is_dir() {
                            continue;
                        }

                        content = Vec::with_capacity(file.size() as usize);
                        let bytes = file.read_to_end(&mut content).map_err(|e| {
                            let msg = format!(
                                "read zip file to buffer failed! zip={}, chunk={}, inner_file={}, {}",
                                meta.local_path,
                                meta.chunk_id,
                                file.name(),
                                e
                            );
                            error!("{}", msg);
                            BuckyError::new(BuckyErrorCode::IoError, msg)
                        })?;

                        file_path = file.enclosed_name().ok_or_else(|| {
                            let msg = format!(
                                "invalid zip file name! zip={}, inner_file={}",
                                meta.local_path,
                                file.name()
                            );
                            error!("{}", msg);
                            BuckyError::new(BuckyErrorCode::InvalidData, msg)
                        })?.to_owned();

                        if bytes as u64 != file.size() {
                            let msg = format!(
                                "read zip file but length unmatch! zip={}, file={}, len={}, got={}",
                                meta.local_path,
                                file.name(),
                                file.size(),
                                bytes,
                            );
                            error!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
                        }
                    }

                    let file_path = PathBuf::from(&meta.local_path).join(file_path);
                    let data = ObjectArchiveInnerFileData::Buffer(content);
                    self.restorer
                        .restore_file(&file_path, data)
                        .await
                        .map_err(|e| {
                            let msg = format!(
                                "restore key data file failed! zip={}, chunk={}, file={}, {}",
                                meta.local_path,
                                meta.chunk_id,
                                file_path.display(),
                                e
                            );
                            error!("{}", msg);
                            BuckyError::new(e.code(), msg)
                        })?;
                }
            }
        }

        Ok(())
    }
}
