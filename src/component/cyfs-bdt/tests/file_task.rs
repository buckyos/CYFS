use std::{
    sync::{Arc}, 
    io::{Seek, SeekFrom}
};
use async_std::{
    io::ReadExt, 
};
use sha2::Digest;
use cyfs_base::*;
use cyfs_bdt::*;
mod utils;

#[async_std::test]
async fn one_small_file() {
    let ((ln_stack, _), (rn_stack, rn_store)) = utils::local_stack_pair(
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

    let (_, reader) = download_file(
        &*ln_stack, 
        file, 
        None, 
        SingleDownloadContext::desc_streams("".to_owned(), vec![rn_stack.local_const().clone()]), 
    ).await.unwrap();
    async_std::io::copy(reader, async_std::io::sink()).await.unwrap();
    
}



#[async_std::test]
async fn same_chunk_file() {
    let ((ln_stack, _), (rn_stack, rn_store)) = utils::local_stack_pair(
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

    let (_, reader) = download_file(
        &*ln_stack, file, 
        None, 
        SingleDownloadContext::desc_streams("".to_owned(), vec![rn_stack.local_const().clone()]), 
    ).await.unwrap();
    async_std::io::copy(reader, async_std::io::sink()).await.unwrap();
  
}




#[async_std::test]
async fn empty_file() {
    let ((ln_stack, _), (rn_stack, _)) = utils::local_stack_pair(
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

    let (_, reader) = download_file(
        &*ln_stack, file, 
        None, 
        SingleDownloadContext::desc_streams("".to_owned(), vec![rn_stack.local_const().clone()]), 
    ).await.unwrap();
    async_std::io::copy(reader, async_std::io::sink()).await.unwrap();
   
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

    
    let (_, mut reader) = download_file(
        &*ln_stack, 
        file, 
        None, 
        SingleDownloadContext::desc_streams("".to_owned(), vec![rn_stack.local_const().clone()]), 
    ).await.unwrap();
    
    
    {
        let mut hasher = sha2::Sha256::new(); 
        let mut buffer = vec![0u8; (range.end - range.start) as usize];
        reader.seek(SeekFrom::Start(range.start)).unwrap();
        reader.read_exact(&mut buffer[..]).await.unwrap();
        hasher.input(&buffer[..]);

        reader.seek(SeekFrom::Start(1024 * 1024 + range.start)).unwrap();
        reader.read_exact(&mut buffer[..]).await.unwrap();
        hasher.input(&buffer[..]);

        assert_eq!(range_hash.result(), hasher.result());
    }
    
}


#[async_std::test]
async fn split_read_file() {
    let ((ln_stack, _), (rn_stack, rn_store)) = utils::local_stack_pair(
        &["W4udp127.0.0.1:10008"], 
        &["W4udp127.0.0.1:10009"]
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
        ChunkList::ChunkInList(chunks.clone())
    ).no_create_time().build();

    let (_, mut reader) = download_file(
        &*ln_stack, 
        file, 
        None, 
        SingleDownloadContext::desc_streams("".to_owned(), vec![rn_stack.local_const().clone()]), 
    ).await.unwrap();

    let mut buffer = vec![0u8; 1024 * 1024];
    for i in 0..2 {
        let chunk_len = 1024 * 1024;
        let (cache, read_range) = reader.split_read(&mut buffer).await.unwrap().unwrap();
        let chunk_hash = hash_data(&buffer[..]);
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
        assert_eq!(cache.chunk(), &chunkid);
        assert_eq!(cache.chunk(), &chunks[i]);
        assert_eq!(read_range.start, 0);
        assert_eq!(read_range.end, chunk_len);
    }

}
