// use std::{
//     sync::{Arc, RwLock}, usize, 
//     io::SeekFrom, 
// };
// use async_std::{
//     pin::Pin, 
//     task::{Context, Poll},
//     fs::File
// };
// use cyfs_base::*;
// use cyfs_util::{
//     AsyncWriteWithSeek, 
//     AsyncReadWithSeek, 
//     SyncWriteWithSeek, 
//     SyncReadWithSeek
// };
// use super::{
//     common::*, 
//     manager::*
// };

// enum CacheState {
//     Creating,
//     Created(File), 
// }

// struct CacheImpl {
//     state: 
//     capacity: usize
// }

// impl CacheImpl {
//     fn capacity(&self) -> usize {
//         self.capacity
//     } 
// }

// impl Drop for FileCache {
//     fn drop(&mut self) {
//         task::spawn(async move {

//         });
//         self.manager.release_mem(self.capacity())
//     }
// }

// #[derive(Clone)]
// struct FileCache(Arc<CacheImpl>);

// #[derive(Clone)]
// pub struct FileCacheGuard(Arc<FileCache>);

// impl FileCacheGuard {
//     pub fn new(manager: RawCacheManager, capacity: usize) -> Self {
//         Self(Arc::new(CacheImpl {
//             cache: RwLock::new(vec![0u8; capacity])
//         }))
//     }
// }

// #[async_trait::async_trait]
// impl RawCache for FileCacheGuard {
//     fn capacity(&self) -> usize {
//         self.0.capacity()
//     }

//     fn clone_as_raw_cache(&self) -> Box<dyn RawCache> {
//         Box::new(self.clone())
//     }

//     async fn async_reader(&self) -> BuckyResult<Box<dyn Unpin + Send + Sync + AsyncReadWithSeek>> {
//         Ok(Box::new(SeekWrapper::new(self)))
//     }

//     fn sync_reader(&self) -> BuckyResult<Box<dyn SyncReadWithSeek>> {
//         Err(BuckyError::new(BuckyErrorCode::NotSupport, "file cache does not support sync reader"))
//     }
    
//     async fn async_writer(&self) -> BuckyResult<Box<dyn  Unpin + Send + Sync + AsyncWriteWithSeek>> {
//         Ok(Box::new(SeekWrapper::new(self)))
//     }   

//     fn sync_writer(&self) -> BuckyResult<Box<dyn SyncWriteWithSeek>> {
//         Err(BuckyError::new(BuckyErrorCode::NotSupport, "file cache does not support sync reader"))
//     }
// }


