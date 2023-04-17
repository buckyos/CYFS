use std::collections::HashMap;
use std::io::{Error, ErrorKind};

use cyfs_base::{BuckyResult, ChunkId};

use std::path::PathBuf;
use async_std::fs::File;
use async_std::io::{BufRead, BufReader};

use lazy_static::lazy_static;
use async_std::sync::Mutex;
use async_std::sync::RwLock;

use std::io::prelude::*;
use crate::chunk_processor::set_chunk;


#[derive(Debug)]
pub struct ChunkLockManager {
    create_lock: RwLock<u32>,
    locks: HashMap<String,RwLock<String>>,
    size: u32,
}

impl ChunkLockManager {
    pub fn new()->ChunkLockManager{
        ChunkLockManager{
            create_lock: RwLock::<u32>::new(1),
            locks: HashMap::new(),
            size: 0
        }
    }

    pub async fn get(&mut self, chunk_id: &ChunkId) -> &mut RwLock<String> {
        self.create_lock.write().await;

        let chunk_str = chunk_id.to_string();

        // while(self.locks.keys().len()>200){
        //     let mut rng = thread_rng();
        //     let x = rng.gen_range(0, self.locks.keys().len());
        //     let key = self.locks.keys().nth(x).unwrap().clone();
        //     if self.locks.contains_key(&key) {
        //         self.locks.remove(&key);
        //         self.size = self.size - 1;
        //     }
        // }

        if self.locks.contains_key(&chunk_str) {
            return self.locks.get_mut(&chunk_str).unwrap();
        } 

        let chunk_str_2 = chunk_str.clone();
        let chunk_str_3 = chunk_str.clone();
        let lock = RwLock::<String>::new(chunk_str_2);
        self.locks.insert(chunk_str_3, lock);
        self.size = self.size + 1;

        return self.locks.get_mut(&chunk_str).unwrap();
    }
}

lazy_static! {
    pub static ref CHUNK_LOCK_MANAGER_SINGLETON: Mutex<ChunkLockManager> = {
        return Mutex::new(ChunkLockManager::new());
    };
}

#[derive(Debug)]
pub struct ChunkStore {
    chunk_dir: PathBuf,
}

impl ChunkStore {
    pub fn new(chunk_dir: &PathBuf) -> ChunkStore {
        ChunkStore{
            chunk_dir: PathBuf::from(chunk_dir),
        }
    }

    pub async fn get(&self, chunk_id: &ChunkId) -> Result<impl BufRead + Unpin, Error> {

        let mut chunk_lock_manager = CHUNK_LOCK_MANAGER_SINGLETON.lock().await;
        let chunk_lock = chunk_lock_manager.get(&chunk_id).await;

        chunk_lock.read().await;

        let chunk_path = self.chunk_dir.join(chunk_id.to_string());
        let file_len = std::fs::metadata(&chunk_path)?.len();
        let chunk_len = chunk_id.len();
        if chunk_len as u64 != file_len {
            error!("file {} len mismatch!, except {} actual {}", chunk_path.display(), chunk_len, file_len);
            info!("delete chunk file {}", chunk_path.display());
            let _ = std::fs::remove_file(chunk_path);
            return Err(Error::from(ErrorKind::NotFound));
        }
        let file = File::open(chunk_path.as_path()).await?;
        let reader = BufReader::new(file);
        Ok(reader)
    }

    pub fn delete(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        let chunk_path = self.chunk_dir.join(chunk_id.to_string());
        if chunk_path.exists() {
            std::fs::remove_file(chunk_path)?;
        }
        Ok(())
    }

    pub async fn set(&self, chunk_id: &ChunkId, chunk: &[u8])->Result<(), Error>{
        let mut chunk_lock_manager = CHUNK_LOCK_MANAGER_SINGLETON.lock().await;
        let chunk_lock = chunk_lock_manager.get(&chunk_id).await;

        chunk_lock.write().await;

        let chunk_path = self.chunk_dir.join(chunk_id.to_string());

        info!("[set_chunk], write chunk {} to {}", chunk_id, chunk_path.display());
        if let Err(e) = std::fs::write(&chunk_path, chunk) {
            error!("set chunk {} err {}", chunk_path.display(), e);
            if chunk_path.exists() {
                info!("delete chunk file {}", chunk_path.display());
                std::fs::remove_file(&chunk_path)?;
            }
            return Err(e)
        }

        info!("[set_chunk], end write chunk file {}", chunk_path.display());
        Ok(())
    }
}