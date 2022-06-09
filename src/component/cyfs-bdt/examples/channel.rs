use async_std::{future, io::prelude::*, task};
use cyfs_base::*;
use cyfs_util::cache::{NamedDataCache, TrackerCache};
use cyfs_bdt::{
    download::*,
    ChunkDownloadConfig,
    ChunkReader,
    ChunkWriter,
    MemChunkStore,
    MemTracker,
    // StackConfig,
    Stack,
    StackGuard,
    StackOpenParams,
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

#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example-channel");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-channel")
        .level("trace")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-channel", "bdt-example-channel")
        .exit_on_panic(true)
        .build()
        .start();

    let (ln_dev, ln_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10000"],
    )
    .unwrap();
    let (rn_dev, rn_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10001"],
    )
    .unwrap();

    let mut ln_params = StackOpenParams::new("bdt-example-channel-download");
    let ln_tracker = MemTracker::new();
    let ln_store = MemChunkStore::new(NamedDataCache::clone(&ln_tracker).as_ref());
    ln_params.chunk_store = Some(ln_store.clone_as_reader());
    ln_params.ndc = Some(NamedDataCache::clone(&ln_tracker));
    ln_params.tracker = Some(TrackerCache::clone(&ln_tracker));

    ln_params.known_device = Some(vec![rn_dev.clone()]);
    let ln_stack = Stack::open(ln_dev.clone(), ln_secret, ln_params)
        .await
        .unwrap();

    let mut rn_params = StackOpenParams::new("bdt-example-channel-upload");
    rn_params.config.interface.udp.sim_loss_rate = 10;
    let rn_stack = Stack::open(rn_dev, rn_secret, rn_params).await.unwrap();

    for _ in 1..2 {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 16 * 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);

        let dir = cyfs_util::get_named_data_root("bdt-example-channel-upload");
        let path = dir.join(chunkid.to_string().as_str());
        let _ = track_chunk_to_path(&*rn_stack, &chunkid, Arc::new(chunk_data), path.as_path())
            .await
            .unwrap();

        let _ = download_chunk(
            &*ln_stack,
            chunkid.clone(),
            ChunkDownloadConfig::force_stream(rn_stack.local_device_id().clone()),
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
    }

    task::sleep(Duration::from_secs(10000000000)).await;
}
