use crate::*;
use cyfs_base::*;

use async_std::sync::{Mutex, MutexGuard};
use futures::{AsyncReadExt, AsyncSeekExt};
use cyfs_sha2::Digest;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait FileObjectBuilderState: Send + Clone {
    async fn get_cur_state(&self) -> BuckyResult<(u64, (u64, [u32; 8]), Vec<ChunkId>)>;
    async fn update(
        &mut self,
        pos: u64,
        hash_state: (u64, &[u32; 8]),
        chunk_id: ChunkId,
    ) -> BuckyResult<()>;
}

#[derive(Clone)]
pub struct FileObjectBuilderStateWrapper<T: FileObjectBuilderState> {
    state: Arc<Mutex<T>>,
}

impl<T: FileObjectBuilderState> FileObjectBuilderStateWrapper<T> {
    pub fn new(state: T) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    async fn get_cur_state(&self) -> BuckyResult<(u64, (u64, [u32; 8]), Vec<ChunkId>)> {
        let t = self.state.lock().await;
        t.get_cur_state().await
    }

    async fn update(
        &self,
        pos: u64,
        hash_state: (u64, &[u32; 8]),
        chunk_id: ChunkId,
    ) -> BuckyResult<()> {
        let mut t = self.state.lock().await;
        t.update(pos, hash_state, chunk_id).await
    }

    pub async fn get_state(&self) -> T {
        let t = self.state.lock().await;
        t.clone()
    }

    pub async fn get_state_mut(&self) -> MutexGuard<'_, T> {
        self.state.lock().await
    }
}

pub struct FileObjectBuilder<T: FileObjectBuilderState> {
    local_path: String,
    owner: ObjectId,
    chunk_size: u32,
    state: Option<FileObjectBuilderStateWrapper<T>>,
}

impl<T: FileObjectBuilderState> FileObjectBuilder<T> {
    pub fn new(
        local_path: String,
        owner: ObjectId,
        chunk_size: u32,
        state: Option<FileObjectBuilderStateWrapper<T>>,
    ) -> Self {
        Self {
            local_path,
            owner,
            chunk_size,
            state,
        }
    }

    async fn get_file_time(path: &Path) -> BuckyResult<(u64, u64, u64)> {
        let metadata = async_std::fs::metadata(path).await?;
        let modify_time = metadata.modified()?;
        let create_time = match metadata.created() {
            Ok(create_time) => create_time,
            Err(_) => modify_time,
        };
        let modify_time = system_time_to_bucky_time(&modify_time);
        let create_time = system_time_to_bucky_time(&create_time);
        let access_time = metadata.accessed()?;
        let access_time = system_time_to_bucky_time(&access_time);
        Ok((create_time, modify_time, access_time))
    }

    pub async fn build(&self) -> BuckyResult<File> {
        let path = Path::new(self.local_path.as_str());
        if !path.is_file() {
            let msg = format!("{} is not file", self.local_path.as_str());
            log::error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        if self.chunk_size % 64 != 0 {
            let msg = format!("chunk size {} mod 64 is not zero", self.chunk_size);
            log::error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let mut file = async_std::fs::File::open(self.local_path.as_str())
            .await
            .map_err(|e| {
                let msg = format!(
                    "open file for calc chunk list error! file={}, {}",
                    self.local_path.as_str(),
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let (pos, hash_state, mut list) = {
            if self.state.is_some() {
                self.state.as_ref().unwrap().get_cur_state().await?
            } else {
                (0, (0, [0u32; 8]), Vec::new())
            }
        };
        let mut file_sha256 = if pos == 0 {
            cyfs_sha2::Sha256::new()
        } else {
            file.seek(SeekFrom::Start(pos)).await.map_err(|e| {
                let msg = format!(
                    "seek file {} to {} failed.{}",
                    self.local_path.as_str(),
                    pos,
                    e
                );
                log::error!("{}", msg.as_str());
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
            cyfs_sha2::Sha256::from((hash_state.0, &hash_state.1))
        };

        let mut file_len = pos as usize;
        let mut file_hash = None;
        let mut buf = Vec::with_capacity(self.chunk_size as usize);

        unsafe {
            buf.set_len(self.chunk_size as usize);
        }
        loop {
            let len = file.read(&mut buf).await.map_err(|e| {
                let msg = format!("read file {} failed.{}", self.local_path.as_str(), e);
                log::error!("{}", msg.as_str());
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
            if len == 0 {
                break;
            }

            let hash = hash_data(&buf[0..len]);
            let chunk_id = ChunkId::new(&hash, len as u32);

            debug!(
                "got file chunk: id={}, len={}, file={}, ",
                chunk_id,
                len,
                self.local_path.as_str()
            );
            list.push(chunk_id.clone());
            file_len += len;

            // 判断是不是最后一个chunk
            if len < self.chunk_size as usize {
                if file_len == len {
                    // 只有一个block的情况，不需要再hash一次了
                    assert!(file_hash.is_none());
                    file_hash = Some(hash);
                } else {
                    file_sha256.input(&buf[0..len]);
                }
                break;
            }

            file_sha256.input(&buf[0..len]);

            if self.state.is_some() {
                self.state
                    .as_ref()
                    .unwrap()
                    .update(file_len as u64, file_sha256.get_state(), chunk_id)
                    .await?;
            }
        }

        let file_hash: HashValue = match file_hash {
            Some(v) => v,
            None => file_sha256.result().into(),
        };

        log::info!("file_hash {}", file_hash.to_string());
        let (create_time, _, _) = Self::get_file_time(Path::new(self.local_path.as_str())).await?;
        let file = File::new(
            self.owner.clone(),
            file_len as u64,
            file_hash,
            ChunkList::ChunkInList(list),
        )
        .create_time(create_time)
        .build();
        Ok(file)
    }
}
