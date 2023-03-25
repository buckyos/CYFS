use crate::crypto::*;
use cyfs_backup_lib::*;
use cyfs_base::*;

use std::path::Path;

pub struct ObjectArchiveIndexHelper;

impl ObjectArchiveIndexHelper {
    pub fn new(id: String, format: ObjectPackFormat, strategy: ObjectBackupStrategy, data_folder: Option<String>) -> ObjectArchiveIndex {
        let datetime = chrono::offset::Local::now();
        let time = format!("{:?}", datetime);

        ObjectArchiveIndex {
            id,
            time,

            format,
            strategy,

            device_id: DeviceId::default(),
            owner: None,
            crypto: CryptoMode::None,
            en_device_id: None,

            data_folder,
            
            object_files: vec![],
            chunk_files: vec![],
            meta: None,
        }
    }

    pub fn init_device_id(
        index: &mut ObjectArchiveIndex,
        device_id: DeviceId,
        owner: Option<ObjectId>,
        crypto: Option<&AesKey>,
    ) {
        let mode;
        let en_device_id;
        match crypto {
            Some(aes_key) => {
                mode = CryptoMode::AES;
                en_device_id = Some(AesKeyHelper::encrypt_device_id(&aes_key, &device_id));
            }
            None => {
                mode = CryptoMode::None;
                en_device_id = None;
            }
        }

        index.device_id = device_id;
        index.owner = owner;
        index.crypto = mode;
        index.en_device_id = en_device_id;
    }

    pub async fn load(dir: &Path) -> BuckyResult<ObjectArchiveIndex> {
        let index_file = dir.join("index");
        let s = async_std::fs::read_to_string(&index_file)
            .await
            .map_err(|e| {
                let msg = format!(
                    "load index info from file failed! file={}, {}",
                    index_file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let ret: ObjectArchiveIndex = serde_json::from_str(&s).map_err(|e| {
            let msg = format!(
                "invalid index info format! file={}, content={}, {}",
                index_file.display(),
                s,
                e,
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        debug!(
            "load backup archive index: file={}, {}",
            index_file.display(),
            s
        );
        Ok(ret)
    }

    pub async fn save(index: &ObjectArchiveIndex, dir: &Path) -> BuckyResult<()> {
        let index_file = dir.join("index");

        let data = serde_json::to_string_pretty(&index).unwrap();
        async_std::fs::write(&index_file, &data)
            .await
            .map_err(|e| {
                let msg = format!(
                    "write backup index info to file failed! file={}, {}",
                    index_file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        info!(
            "save backup index success! index={}, file={}",
            data,
            index_file.display()
        );

        Ok(())
    }
}
