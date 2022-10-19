use async_std::{future, io::prelude::*, task};
use cyfs_base::*;
use cyfs_bdt::{
    download::*, SingleDownloadContext, ChunkWriter, StackGuard, 
};
use std::{sync::Arc, time::Duration};
mod utils;

async fn watch_recv_chunk(stack: StackGuard, chunkid: ChunkId) -> BuckyResult<ChunkId> {
    loop {
        let ret = stack.ndn().chunk_manager().store().read(&chunkid).await;
        if let Ok(mut reader) = ret {
            let mut content = vec![0u8; chunkid.len()];
            let _ = reader.read(content.as_mut_slice()).await?;
            let recv_id = ChunkId::calculate(content.as_slice()).await?;
            return Ok(recv_id);
        } else {
            task::sleep(Duration::from_millis(500)).await;
        }
    }
}

async fn watch_release_uploader(_stack: StackGuard, _chunkid: &ChunkId) {
    // loop {
    //     if stack.ndn().chunk_manager().view_of(&chunkid).is_some() {
    //         task::sleep(Duration::from_millis(500)).await;
    //     } else {
    //         break;
    //     }
    // }
}



#[async_std::test]
async fn one_task_group() {
    let ((ln_stack, ln_store), (rn_stack, rn_store)) =
        utils::local_stack_pair_with_config(&["W4udp127.0.0.1:10000"], &["W4udp127.0.0.1:10001"], None, None)
            .await
            .unwrap();

    {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
    
        let _ = rn_store
            .add(chunkid.clone(), Arc::new(chunk_data))
            .await
            .unwrap();
    
        let context = SingleDownloadContext::streams(None, vec![rn_stack.local_device_id().clone()]);
    
        let _ = download_chunk(
            &*ln_stack,
            chunkid.clone(), 
            Some("test-group::".to_owned()), 
            Some(context),
            vec![ln_store.clone_as_writer()],
        )
        .await;
        let recv = future::timeout(
            Duration::from_secs(5),
            watch_recv_chunk(ln_stack.clone(), chunkid.clone()),
        )
        .await
        .unwrap();
        let recv_chunk_id = recv.unwrap();
        assert_eq!(recv_chunk_id, chunkid);
    
        let _ = future::timeout(
            Duration::from_secs(5),
            watch_release_uploader(rn_stack.clone(), &chunkid),
        )
        .await
        .unwrap();
    }


    {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
    
        let _ = rn_store
            .add(chunkid.clone(), Arc::new(chunk_data))
            .await
            .unwrap();
    
        let _ = download_chunk(
            &*ln_stack,
            chunkid.clone(), 
            Some("test-group::2".to_owned()), 
            None, 
            vec![ln_store.clone_as_writer()],
        )
        .await;

        let group = get_download_task(&*ln_stack, "test-group").unwrap();

        let _ = group.sub_task("2").unwrap();

        let recv = future::timeout(
            Duration::from_secs(5),
            watch_recv_chunk(ln_stack.clone(), chunkid.clone()),
        )
        .await
        .unwrap();
        let recv_chunk_id = recv.unwrap();
        assert_eq!(recv_chunk_id, chunkid);
    
        let _ = future::timeout(
            Duration::from_secs(5),
            watch_release_uploader(rn_stack.clone(), &chunkid),
        )
        .await
        .unwrap();
    }


}
