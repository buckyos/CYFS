use std::{
    sync::Arc, 
    time::Duration, 
    ops::Range
};
use async_std::{
    future, 
};
use async_trait::async_trait;
use sha2::Digest;
use cyfs_base::*;
use cyfs_bdt::{
    DownloadTask, 
    DownloadTaskState, 
    SingleDownloadContext, 
    ChunkWriter, 
    ChunkWriterExt, 
    download::*
};
use cyfs_debug::Mutex;
mod utils;

async fn watch_task_finish(task: Box<dyn DownloadTask>) -> BuckyResult<()> {
    loop {
        match task.state() {
            DownloadTaskState::Finished => {
                break Ok(());
            },
            _ => {}
        }
    }
}

#[async_std::test]
async fn one_small_file() {
    let ((ln_stack, ln_store), (rn_stack, rn_store)) = utils::local_stack_pair(
        &["W4udp127.0.0.1:10000"], 
        &["W4udp127.0.0.1:10001"]
    ).await.unwrap();
    
    let mut file_hash  = sha2::Sha256::new();
    let mut file_len = 0u64;
    let mut chunks = vec![];
    for _ in 0..2 {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        file_hash.input(&chunk_data[..]);
        file_len += chunk_len as u64;
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
        let _ = rn_store.add(chunkid.clone(), Arc::new(chunk_data)).await.unwrap();
        chunks.push(chunkid);
    }

    let file = File::new(
        ObjectId::default(),
        file_len,
        file_hash.result().into(),
        ChunkList::ChunkInList(chunks)
    ).no_create_time().build();

    let task = download_file(
        &*ln_stack, 
        file, 
        None, 
        Some(SingleDownloadContext::streams(None, vec![rn_stack.local_device_id().clone()])), 
        vec![ln_store.clone_as_writer()]).await.unwrap();
    let recv = future::timeout(Duration::from_secs(5), watch_task_finish(task)).await.unwrap();
    let _ = recv.unwrap();
}



#[async_std::test]
async fn same_chunk_file() {
    let ((ln_stack, ln_store), (rn_stack, rn_store)) = utils::local_stack_pair(
        &["W4udp127.0.0.1:10002"], 
        &["W4udp127.0.0.1:10003"]).await.unwrap();
    
    let mut file_hash  = sha2::Sha256::new();
    let mut file_len = 0u64;

    let (chunk_len, chunk_data) = utils::random_mem(1024, 512);
    let chunk_hash = hash_data(&chunk_data[..]);
    let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);

    let mut chunks = vec![];
    for _ in 0..2 {
        file_hash.input(&chunk_data[..]);
        file_len += chunk_len as u64;
        chunks.push(chunkid.clone());
    }

    let _ = rn_store.add(chunkid.clone(), Arc::new(chunk_data)).await.unwrap();

    let file = File::new(
        ObjectId::default(),
        file_len,
        file_hash.result().into(),
        ChunkList::ChunkInList(chunks)
    ).no_create_time().build();

    let task = download_file(
        &*ln_stack, file, 
        None, 
        Some(SingleDownloadContext::streams(None, vec![rn_stack.local_device_id().clone()])), 
        vec![ln_store.clone_as_writer()]).await.unwrap();
    let recv = future::timeout(Duration::from_secs(5), watch_task_finish(task)).await.unwrap();
    let _ = recv.unwrap();
}




#[async_std::test]
async fn empty_file() {
    let ((ln_stack, ln_store), (rn_stack, _)) = utils::local_stack_pair(
        &["W4udp127.0.0.1:10004"], 
        &["W4udp127.0.0.1:10005"]).await.unwrap();
    
    let (file_len, file_data) = (0, vec![0u8; 0]);
    let file_hash = hash_data(&file_data[..]);

    let file = File::new(
        ObjectId::default(),
        file_len,
        file_hash,
        ChunkList::ChunkInList(vec![])
    ).no_create_time().build();

    let task = download_file(
        &*ln_stack, file, 
        None, 
        Some(SingleDownloadContext::streams(None, vec![rn_stack.local_device_id().clone()])), 
        vec![ln_store.clone_as_writer()]).await.unwrap();
    let recv = future::timeout(Duration::from_secs(5), watch_task_finish(task)).await.unwrap();
    let _ = recv.unwrap();
}




#[async_std::test]
async fn one_small_file_with_ranges() {
    let ((ln_stack, _), (rn_stack, rn_store)) = utils::local_stack_pair(
        &["W4udp127.0.0.1:10006"], 
        &["W4udp127.0.0.1:10007"]
    ).await.unwrap();
    
    let mut range_hash = sha2::Sha256::new();
    let range = 1000u64..1024u64;
    let mut file_hash  = sha2::Sha256::new();
    let mut file_len = 0u64;
    let mut chunks = vec![];
    for _ in 0..2 {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        file_hash.input(&chunk_data[..]);
        range_hash.input(&chunk_data[range.start as usize..range.end as usize]);
        file_len += chunk_len as u64;
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
        let _ = rn_store.add(chunkid.clone(), Arc::new(chunk_data)).await.unwrap();
        chunks.push(chunkid);
    }

    let file = File::new(
        ObjectId::default(),
        file_len,
        file_hash.result().into(),
        ChunkList::ChunkInList(chunks)
    ).no_create_time().build();


    
    #[derive(Clone)]
    struct RangeWriter(Arc<WriterImpl>);

    struct WriterImpl {
        hasher: Mutex<sha2::Sha256>, 
        hash: HashValue
    }

    impl RangeWriter {
        fn new(hash: HashValue) -> Self {
            Self(Arc::new(WriterImpl {
                hasher: Mutex::new(sha2::Sha256::new()), 
                hash
            }))
        }
    }

    impl std::fmt::Display for RangeWriter {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "RangeWriter")
        }
    }

    #[async_trait]
    impl ChunkWriterExt for RangeWriter {
        fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt> {
            Box::new(self.clone())
        }
        
        async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>, range: Option<Range<u64>>) -> BuckyResult<()> {
            let range = range.unwrap_or(0..chunk.len() as u64);
            self.0.hasher.lock().unwrap().input(&content.as_slice()[range.start as usize..range.end as usize]);
            Ok(())
        }

        async fn finish(&self) -> BuckyResult<()> {
            assert_eq!(self.0.hash, self.0.hasher.lock().unwrap().clone().result().into());
            Ok(())
        }

        async fn err(&self, _e: BuckyErrorCode) -> BuckyResult<()> {
            unreachable!()
        }
    }
    
    let task = download_file_with_ranges(
        &*ln_stack, 
        file, 
        Some(vec![range.clone(), range.start + 1024 * 1024..range.end + 1024 * 1024]), 
        None, 
        Some(SingleDownloadContext::streams(None, vec![rn_stack.local_device_id().clone()])), 
        vec![RangeWriter::new(range_hash.result().into()).clone_as_writer()]).await.unwrap();
    let recv = future::timeout(Duration::from_secs(5), watch_task_finish(task)).await.unwrap();
    let _ = recv.unwrap();
}