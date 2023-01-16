use cyfs_base::*;
use cyfs_chunk_cache::{CachedFile, ChunkManager, ChunkType};
use cyfs_chunk_lib::ChunkRead;
use cyfs_task_manager::*;

use async_std::io::prelude::SeekExt;
use async_std::io::{Read, ReadExt};
use cyfs_debug::Mutex;
use sha2::Digest;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

pub struct VerifyFileRunnable {
    chunk_manager: Arc<ChunkManager>,
    task_id: TaskId,
    file: Option<File>,
    chunk_id: Option<ChunkId>,
    save_path: Option<String>,
    verify_result: Mutex<bool>,
}

impl VerifyFileRunnable {
    pub fn new(
        chunk_manager: Arc<ChunkManager>,
        task_id: TaskId,
        file: Option<File>,
        chunk_id: Option<ChunkId>,
        save_path: Option<String>,
    ) -> Self {
        assert!(file.is_some() || chunk_id.is_some());

        Self {
            chunk_manager,
            task_id,
            file,
            chunk_id,
            save_path,
            verify_result: Mutex::new(false),
        }
    }

    async fn verify_chunk(
        &self,
        chunk_id: &ChunkId,
        buf: &mut [u8],
        mut reader: impl Read + Unpin + Send + Sync + 'static,
    ) -> BuckyResult<()> {
        let mut sha256 = sha2::Sha256::new();

        let chunk_len = chunk_id.len();
        let mut read_len = 0;
        loop {
            match reader.read(buf).await {
                Ok(size) => {
                    if size == 0 {
                        break;
                    }
                    sha256.input(&buf[0..size]);
                    read_len += size;
                    if read_len >= chunk_len {
                        break;
                    }
                }
                Err(e) => {
                    let msg = format!(
                        "verify chunk but read from chunkmanager failed! task={}, chunk={}, {}",
                        self.task_id, chunk_id, e
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
                }
            }
        }

        let real_hash: HashValue = sha256.result().into();
        let real_chunk_id = ChunkId::new(&real_hash, chunk_len as u32);
        if real_chunk_id != *chunk_id {
            let msg = format!(
                "verify chunk but not match! task={}, len={}, expect={}, got={}",
                self.task_id, chunk_len, chunk_id, real_chunk_id,
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::Unmatch, msg))
        } else {
            // println!("chunk match: {}", real_chunk_id);
            Ok(())
        }
    }

    async fn verify_chunk_list_from_chunk_manager(
        &self,
        chunk_list: &[ChunkId],
    ) -> BuckyResult<()> {
        let mut buf = Vec::with_capacity(1024 * 64);
        unsafe {
            buf.set_len(1024 * 64);
        }

        for chunk_id in chunk_list {
            match self
                .chunk_manager
                .get_chunk(chunk_id, ChunkType::MMapChunk)
                .await
            {
                Ok(chunk) => {
                    let reader = ChunkRead::new(chunk);
                    self.verify_chunk(&chunk_id, &mut buf, reader).await?;
                }
                Err(e) => {
                    let msg = format!(
                        "verify chunklist but got chunk from chunkmanager failed! task={}, chunk={}, {}",
                        self.task_id, chunk_id, e
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(e.code(), msg));
                }
            }
        }

        Ok(())
    }

    async fn verify_chunk_list_from_file(
        &self,
        chunk_list: &[ChunkId],
        path: &Path,
    ) -> BuckyResult<()> {
        let mut buf = Vec::with_capacity(1024 * 64);
        unsafe {
            buf.set_len(1024 * 64);
        }

        if !path.exists() {
            let msg = format!(
                "verify task local file but not exists! task={}, local_file={}",
                self.task_id.to_string(),
                self.save_path.as_ref().unwrap()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let mut file = async_std::fs::File::open(path).await.map_err(|e| {
            let msg = format!(
                "verify task local file but open failed! task={}, local_file={}, {}",
                self.task_id.to_string(),
                self.save_path.as_ref().unwrap(),
                e,
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let mut pos = 0;
        for chunk_id in chunk_list {
            file.seek(SeekFrom::Start(pos)).await.map_err(|e| {
                let msg = format!(
                    "verify task local file but seek failed! task={}, local_file={}, {}",
                    self.task_id.to_string(),
                    self.save_path.as_ref().unwrap(),
                    e,
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
            pos += chunk_id.len() as u64;
            let reader = file.clone().take(chunk_id.len() as u64);

            self.verify_chunk(chunk_id, &mut buf, reader).await?;
        }

        Ok(())
    }

    async fn verify(&self) -> BuckyResult<()> {
        if let Some(file) = &self.file {
            self.verify_file(file).await
        } else if let Some(chunk_id) = &self.chunk_id {
            self.verify_chunk_id(chunk_id).await
        } else {
            unreachable!();
        }
    }

    async fn verify_chunk_id(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        info!(
            "will verify chunk: task={}, chunk={}, save_path={:?}",
            self.task_id, chunk_id, self.save_path
        );

        let chunk_list = vec![chunk_id.to_owned()];
        if self.save_path.is_some() && !self.save_path.as_ref().unwrap().is_empty() {
            let path = PathBuf::from_str(self.save_path.as_ref().unwrap()).unwrap();
            self.verify_chunk_list_from_file(&chunk_list, &path).await
        } else {
            self.verify_chunk_list_from_chunk_manager(&chunk_list).await
        }
    }

    async fn verify_file(&self, file: &File) -> BuckyResult<()> {
        info!(
            "will verify file: task={}, file={}, save_path={:?}",
            self.task_id,
            file.desc().calculate_id(),
            self.save_path
        );

        match file.body() {
            Some(body) => match body.content().chunk_list() {
                ChunkList::ChunkInBundle(_) => self.verify_bundle(file).await,
                _ => self.verify_list(file).await,
            },
            None => Ok(()),
        }
    }

    async fn verify_bundle(&self, file: &File) -> BuckyResult<()> {
        let chunk_list = match file
            .body_expect("invalid file object body")
            .content()
            .chunk_list()
        {
            ChunkList::ChunkInBundle(bundle) => bundle.chunk_list(),
            _ => unreachable!(),
        };

        if self.save_path.is_some() && !self.save_path.as_ref().unwrap().is_empty() {
            let path = PathBuf::from_str(self.save_path.as_ref().unwrap()).unwrap();
            self.verify_chunk_list_from_file(chunk_list, &path).await
        } else {
            self.verify_chunk_list_from_chunk_manager(chunk_list).await
        }
    }

    // 校验一下task本地文件是否匹配
    async fn verify_list(&self, file: &File) -> BuckyResult<()> {
        let result = loop {
            if self.save_path.is_some() && !self.save_path.as_ref().unwrap().is_empty() {
                let path = PathBuf::from_str(self.save_path.as_ref().unwrap()).unwrap();
                if !path.exists() {
                    error!(
                        "verify task local file but not exists! task={}, local_file={}",
                        self.task_id.to_string(),
                        self.save_path.as_ref().unwrap()
                    );
                    break false;
                }

                // 校验hash
                let ret = hash_file(&path).await;
                if ret.is_err() {
                    error!(
                        "hash file error: task_id={}, local_path={}, {}",
                        self.task_id.to_string(),
                        self.save_path.as_ref().unwrap(),
                        ret.unwrap_err()
                    );
                    break false;
                }

                let len = file.desc().content().len();
                let (real_hash, real_len) = ret.unwrap();
                if real_len != len {
                    error!(
                        "file len not match! task_id={}, local_path={}, expect={}, got={}",
                        self.task_id.to_string(),
                        self.save_path.as_ref().unwrap(),
                        len,
                        real_len
                    );
                    break false;
                }

                if real_hash != *file.desc().content().hash() {
                    error!(
                        "file hash not match! task_id={}, local_path={}, expect={}, got={}",
                        self.task_id.to_string(),
                        self.save_path.as_ref().unwrap(),
                        file.desc().content().hash().to_string(),
                        real_hash
                    );
                    break false;
                }

                info!(
                    "verify local file complete! task_id={}, local_path={}, len={}, hash={}",
                    self.task_id.to_string(),
                    self.save_path.as_ref().unwrap(),
                    len,
                    real_hash
                );
                break true;
            } else {
                let mut cache_file =
                    CachedFile::open(file.clone(), self.chunk_manager.clone()).await?;
                let mut sha256 = sha2::Sha256::new();
                let mut buf = Vec::with_capacity(1024 * 64);
                unsafe {
                    buf.set_len(1024 * 64);
                }
                let mut file_len = 0;
                loop {
                    match cache_file.read(&mut buf).await {
                        Ok(size) => {
                            if size == 0 {
                                break;
                            }
                            sha256.input(&buf[0..size]);
                            file_len = file_len + size;
                        }
                        Err(e) => {
                            return Err(BuckyError::from(e));
                        }
                    }
                }

                let real_hash: HashValue = sha256.result().into();
                if real_hash != *file.desc().content().hash() {
                    error!(
                        "file hash not match! task_id={}, local_path={}, expect={}, got={}",
                        self.task_id.to_string(),
                        self.save_path.as_ref().unwrap(),
                        file.desc().content().hash().to_string(),
                        real_hash
                    );
                    break false;
                }

                info!(
                    "verify local file complete! task_id={}, local_path={}, len={}, hash={}",
                    self.task_id.to_string(),
                    self.save_path.as_ref().unwrap(),
                    file_len,
                    real_hash
                );
                break true;
            }
        };

        if !result {
            let msg = format!(
                "verify task local files but invalid! task={}",
                self.task_id.to_string()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Runnable for VerifyFileRunnable {
    fn get_task_id(&self) -> TaskId {
        self.task_id.clone()
    }

    fn get_task_type(&self) -> TaskType {
        TaskType(0)
    }

    fn get_task_category(&self) -> TaskCategory {
        TaskCategory(0)
    }

    async fn set_task_store(&mut self, _task_store: Arc<dyn TaskStore>) {}

    async fn run(&self) -> BuckyResult<()> {
        match self.verify().await {
            Ok(()) => {
                *self.verify_result.lock().unwrap() = true;
            }
            Err(_) => {
                *self.verify_result.lock().unwrap() = false;
            }
        }
        Ok(())
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        let ret: bool = { *self.verify_result.lock().unwrap() };
        Ok(ret.to_vec()?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::{Path, PathBuf};

    async fn create_chunk_list(
        source: &Path,
        chunk_size: u32,
    ) -> BuckyResult<(HashValue, u64, Vec<ChunkId>)> {
        let mut list = Vec::new();
        let mut file = async_std::fs::File::open(source).await.map_err(|e| {
            let msg = format!(
                "open file for calc chunk list error! file={}, {}",
                source.display(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let mut file_sha256 = sha2::Sha256::new();
        let mut file_len = 0;
        let mut file_hash = None;
        let mut buf = Vec::with_capacity(chunk_size as usize);

        unsafe {
            buf.set_len(chunk_size as usize);
        }
        loop {
            let len = file.read(&mut buf).await?;
            if len == 0 {
                break;
            }

            let hash = cyfs_base::hash_data(&buf[0..len]);
            let chunk_id = ChunkId::new(&hash, len as u32);

            debug!(
                "got file chunk: id={}, len={}, file={}, ",
                chunk_id,
                len,
                source.display()
            );
            list.push(chunk_id);
            file_len += len;

            // 判断是不是最后一个chunk
            if len < chunk_size as usize {
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
        }

        let file_hash: HashValue = match file_hash {
            Some(v) => v,
            None => file_sha256.result().into(),
        };

        Ok((file_hash, file_len as u64, list))
    }

    async fn test_chunk_bundle(chunk_manager: &Arc<ChunkManager>) {
        let path = PathBuf::from("H:\\data");
        let (file_hash, len, list) = create_chunk_list(&path, 1024 * 1024 * 4).await.unwrap();

        let bundle = ChunkBundle::new(list.clone(), ChunkBundleHashMethod::Serial);
        let owner = ObjectId::default();
        let hash = bundle.calc_hash_value();
        let chunk_list = ChunkList::ChunkInBundle(bundle);

        let file = cyfs_base::File::new(owner.clone(), len, hash.clone(), chunk_list)
            .no_create_time()
            .build();

        let task = VerifyFileRunnable {
            chunk_manager: chunk_manager.clone(),
            task_id: TaskId::from(hash.as_slice()),
            file: Some(file),
            chunk_id: None,
            save_path: Some("H:\\data".to_owned()),
            verify_result: Mutex::new(false),
        };
        task.run().await.unwrap();
        assert!(*task.verify_result.lock().unwrap());

        let chunk_list = ChunkList::ChunkInList(list);
        let file = cyfs_base::File::new(owner, len, file_hash, chunk_list)
            .no_create_time()
            .build();

        let task = VerifyFileRunnable {
            chunk_manager: chunk_manager.clone(),
            task_id: TaskId::from(hash.as_slice()),
            file: Some(file),
            chunk_id: None,
            save_path: Some("H:\\data".to_owned()),
            verify_result: Mutex::new(false),
        };
        task.run().await.unwrap();
        assert!(*task.verify_result.lock().unwrap());
    }

    async fn run() {
        let chunk_manager = ChunkManager::new();
        chunk_manager.init("test-verify-file").await.unwrap();
        let chunk_manager = Arc::new(chunk_manager);

        test_chunk_bundle(&chunk_manager).await;
    }

    #[test]
    fn test() {
        async_std::task::block_on(run());
    }
}
