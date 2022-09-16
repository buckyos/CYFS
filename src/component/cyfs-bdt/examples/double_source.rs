use async_std::{future, io::prelude::*, task};
use cyfs_base::*;
use cyfs_util::cache::{NamedDataCache, TrackerCache};
use cyfs_bdt::{
    download::*,
    SingleDownloadContext, 
    ChunkReader,
    ChunkWriter,
    MemChunkStore,
    MemTracker,
    // StackConfig,
    Stack,
    StackGuard,
    StackOpenParams,
    DownloadTask, 
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

fn watch_resource(task: Box<dyn DownloadTask>) {
    task::spawn(async move {
        loop {
            log::info!("task state: {:?}", task.state());
            task::sleep(Duration::from_millis(500)).await;
        }
    });   
}



#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example-double-source");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-double-source")
        .level("trace")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-double-source", "bdt-example-double-source")
        .exit_on_panic(true)
        .build()
        .start();

    let (down_dev, down_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10000"],
    )
    .unwrap();
    let (ref_dev, ref_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10001"],
    )
    .unwrap();
    let (src_dev, src_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10002"],
    )
    .unwrap();

    let (down_stack, down_store) = {
        let mut params = StackOpenParams::new("bdt-example-double-source-download");
        let tracker = MemTracker::new();
        let store = MemChunkStore::new(NamedDataCache::clone(&tracker).as_ref());
        params.chunk_store = Some(store.clone_as_reader());
        params.ndc = Some(NamedDataCache::clone(&tracker));
        params.tracker = Some(TrackerCache::clone(&tracker));
        params.known_device = Some(vec![ref_dev.clone(), src_dev.clone()]);
        (
            Stack::open(down_dev.clone(), down_secret, params)
                .await
                .unwrap(),
            store,
        )
    };

    let (ref_stack, ref_store) = {
        let mut params = StackOpenParams::new("bdt-example-double-source-ref");
        let tracker = MemTracker::new();
        let store = MemChunkStore::new(NamedDataCache::clone(&tracker).as_ref());
        params.chunk_store = Some(store.clone_as_reader());
        params.ndc = Some(NamedDataCache::clone(&tracker));
        params.tracker = Some(TrackerCache::clone(&tracker));
        (
            Stack::open(ref_dev, ref_secret, params).await.unwrap(),
            store,
        )
    };

    let (src_stack, src_store) = {
        let mut params = StackOpenParams::new("bdt-example-double-source-src");
        params.config.interface.udp.sim_loss_rate = 10;
        let tracker = MemTracker::new();
        let store = MemChunkStore::new(NamedDataCache::clone(&tracker).as_ref());
        params.chunk_store = Some(store.clone_as_reader());
        params.ndc = Some(NamedDataCache::clone(&tracker));
        params.tracker = Some(TrackerCache::clone(&tracker));
        (
            Stack::open(src_dev, src_secret, params).await.unwrap(),
            store,
        )
    };

    for _ in 1..2 {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 16 * 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);

        let to_put = Arc::new(chunk_data);
        let _ = ref_store
            .add(chunkid.clone(), to_put.clone())
            .await
            .unwrap();
        let _ = src_store
            .add(chunkid.clone(), to_put.clone())
            .await
            .unwrap();

        let task = download_chunk(
            &*down_stack,
            chunkid.clone(),
            SingleDownloadContext::streams(None, vec![
                ref_stack.local_device_id().clone(),
                src_stack.local_device_id().clone(),]
            ),
            vec![down_store.clone_as_writer()],
        )
        .await.unwrap();

        watch_resource(task);

        let recv = future::timeout(
            Duration::from_secs(5),
            watch_recv_chunk(down_stack.clone(), chunkid.clone()),
        )
        .await
        .unwrap();
        let recv_chunk_id = recv.unwrap();
        assert_eq!(recv_chunk_id, chunkid);
    }

    task::sleep(Duration::from_secs(10000000000)).await;
}
