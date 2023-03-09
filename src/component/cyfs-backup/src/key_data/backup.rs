use super::key_data::*;
use super::zip_helper::ZipHelper;
use crate::data::*;
use crate::meta::{KeyDataMeta, KeyDataType};
use cyfs_base::*;
use cyfs_util::AsyncReadWithSeek;

use std::path::PathBuf;

pub struct KeyDataBackup {
    cyfs_root: PathBuf,
    list: Vec<KeyData>,
    data_writer: BackupDataWriterRef,
}

impl KeyDataBackup {
    pub fn new(cyfs_root: PathBuf, list: Vec<KeyData>, data_writer: BackupDataWriterRef) -> Self {
        Self {
            cyfs_root,
            list,
            data_writer,
        }
    }

    pub async fn run(&self) -> BuckyResult<Vec<KeyDataMeta>> {
        let mut list = Vec::with_capacity(self.list.len());

        for item in &self.list {
            let chunk_id = self.backup_data(item).await?;
            if chunk_id.is_none() {
                continue;
            }

            let chunk_id = chunk_id.unwrap();
            info!(
                "backup key data complete! data={:?}, chunk={}",
                item, chunk_id
            );

            let meta = KeyDataMeta {
                local_path: item.local_path.clone(),
                data_type: item.data_type,
                chunk_id,
            };

            list.push(meta);
        }

        Ok(list)
    }

    async fn backup_data(&self, data: &KeyData) -> BuckyResult<Option<ChunkId>> {
        let file = self.cyfs_root.join(&data.local_path);
        if !file.exists() {
            warn!("target key data not exists! {}", file.display());
            return Ok(None);
        }

        let data = match data.data_type {
            KeyDataType::File => async_std::fs::read(&file).await.map_err(|e| {
                let msg = format!(
                    "read local file to buffer failed! file={}, {}",
                    file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?,
            KeyDataType::Dir => {
                ZipHelper::zip_dir_to_buffer(&file, zip::CompressionMethod::Stored)?
            }
        };

        let chunk_id = ChunkId::calculate_sync(&data).unwrap();
        info!(
            "key_data: file={}, len={}, id={}",
            file.display(),
            data.len(),
            chunk_id
        );

        let cursor = async_std::io::Cursor::new(data);

        let reader = Box::new(cursor) as Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>;

        self.data_writer
            .add_chunk_data(None, None, &chunk_id, reader, None)
            .await?;

        Ok(Some(chunk_id))
    }
}
