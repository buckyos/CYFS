use async_std::{future, io::prelude::*, task};
use cyfs_base::*;
use cyfs_util::cache::{NamedDataCache, TrackerCache};
use cyfs_bdt::{
    *, 
    download::*, 
    ndn::channel::{*, protocol::v0::*},
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

async fn one_small_chunk(ln_ep: &[&str], rn_ep: &[&str], uploader_config: Option<StackConfig>) {
    let ((ln_stack, ln_store), (rn_stack, rn_store)) =
        utils::local_stack_pair_with_config(ln_ep, rn_ep, None, uploader_config)
            .await
            .unwrap();

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
        None, 
        Some(SingleDownloadContext::desc_streams(None, vec![rn_stack.local_const().clone()])),
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

#[async_std::test]
async fn one_small_chunk_udp_channel() {
    one_small_chunk(&["W4udp127.0.0.1:10000"], &["W4udp127.0.0.1:10001"], None).await
}

#[async_std::test]
async fn one_small_chunk_with_loss() {
    let mut uploader_config = StackConfig::new("");
    uploader_config.interface.udp.sim_loss_rate = 10;
    one_small_chunk(
        &["W4udp127.0.0.1:10002"],
        &["W4udp127.0.0.1:10003"],
        Some(uploader_config),
    )
    .await
}

#[async_std::test]
async fn empty_chunk() {
    let ((ln_stack, ln_store), (rn_stack, _)) =
        utils::local_stack_pair(&["W4udp127.0.0.1:10004"], &["W4udp127.0.0.1:10005"])
            .await
            .unwrap();

    let (chunk_len, chunk_data) = (0, vec![0u8; 0]);
    let chunk_hash = hash_data(&chunk_data[..]);
    let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);

    let _ = download_chunk(
        &*ln_stack,
        chunkid.clone(), 
        None, 
        Some(SingleDownloadContext::desc_streams(None, vec![rn_stack.local_const().clone()])), 
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

#[async_std::test]
async fn one_small_chunk_with_refer() {
    let ((ln_stack, ln_store), (rn_stack, rn_store)) = utils::local_stack_pair_with_config(
        &["W4udp127.0.0.1:10006"],
        &["W4udp127.0.0.1:10007"],
        None,
        None,
    )
    .await
    .unwrap();

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
        None, 
        Some(SingleDownloadContext::desc_streams(Some("referer".to_owned()), vec![rn_stack.local_const().clone()])), 
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

#[async_std::test]
async fn one_small_chunk_in_file() {
    let (ln_dev, ln_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10008"],
    )
    .unwrap();
    let (rn_dev, rn_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10009"],
    )
    .unwrap();

    let mut ln_params = StackOpenParams::new("");
    let ln_tracker = MemTracker::new();
    let ln_store = MemChunkStore::new(NamedDataCache::clone(&ln_tracker).as_ref());
    ln_params.chunk_store = Some(ln_store.clone_as_reader());
    ln_params.ndc = Some(NamedDataCache::clone(&ln_tracker));
    ln_params.tracker = Some(TrackerCache::clone(&ln_tracker));

    ln_params.known_device = Some(vec![rn_dev.clone()]);
    let ln_stack = Stack::open(ln_dev.clone(), ln_secret, ln_params)
        .await
        .unwrap();

    let rn_params = StackOpenParams::new("");
    let rn_stack = Stack::open(rn_dev, rn_secret, rn_params).await.unwrap();

    let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
    let chunk_hash = hash_data(&chunk_data[..]);
    let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);

    let dir = cyfs_util::get_named_data_root(rn_stack.local_device_id().to_string().as_str());
    let path = dir.join(chunkid.to_string().as_str());
    let _ = track_chunk_to_path(&*rn_stack, &chunkid, Arc::new(chunk_data), path.as_path())
        .await
        .unwrap();

    let _ = download_chunk(
        &*ln_stack,
        chunkid.clone(), 
        None, 
        Some(SingleDownloadContext::desc_streams(None, vec![rn_stack.local_const().clone()])),
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

#[async_std::test]
async fn one_small_chunk_double_source() {
    let (down_dev, down_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10010"],
    )
    .unwrap();
    let (ref_dev, ref_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10011"],
    )
    .unwrap();
    let (src_dev, src_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10012"],
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

    let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
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

    let context = SingleDownloadContext::desc_streams(None, vec![
        ref_stack.local_const().clone(),
        src_stack.local_const().clone(),
    ]);
    let _ = download_chunk(
        &*down_stack,
        chunkid.clone(), 
        None, 
        Some(context),
        vec![down_store.clone_as_writer()],
    )
    .await;
    let recv = future::timeout(
        Duration::from_secs(5),
        watch_recv_chunk(down_stack.clone(), chunkid.clone()),
    )
    .await
    .unwrap();
    let recv_chunk_id = recv.unwrap();
    assert_eq!(recv_chunk_id, chunkid);

    let _ = future::timeout(
        Duration::from_secs(5),
        watch_release_uploader(ref_stack.clone(), &chunkid),
    )
    .await
    .unwrap();

    let _ = future::timeout(
        Duration::from_secs(5),
        watch_release_uploader(src_stack.clone(), &chunkid),
    )
    .await
    .unwrap();
}

#[async_std::test]
async fn one_small_chunk_tcp_channel() {
    one_small_chunk(&["W4tcp127.0.0.1:10000"], &["W4tcp127.0.0.1:10001"], None).await
}

#[async_std::test]
async fn upload_from_downloader(ln_ep: &[&str], rn_ep: &[&str], uploader_config: Option<StackConfig>) {
    let (down_dev, down_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10013"],
    )
    .unwrap();

    let (cache_dev, cache_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10014"],
    )
    .unwrap();

    let (src_dev, src_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10015"],
    )
    .unwrap();

    let (down_stack, down_store) = {
        let mut params = StackOpenParams::new("bdt-example-upload-from-downloader-down");
        let tracker = MemTracker::new();
        let store = MemChunkStore::new(NamedDataCache::clone(&tracker).as_ref());
        params.chunk_store = Some(store.clone_as_reader());
        params.ndc = Some(NamedDataCache::clone(&tracker));
        params.tracker = Some(TrackerCache::clone(&tracker));
        params.known_device = Some(vec![cache_dev.clone(), src_dev.clone()]);
        (
            Stack::open(down_dev.clone(), down_secret, params)
                .await
                .unwrap(),
            store,
        )
    };

    let (cache_stack, _cache_store) = {
        let mut params = StackOpenParams::new("bdt-example-upload-from-downloader-cache");
        let tracker = MemTracker::new();
        let store = MemChunkStore::new(NamedDataCache::clone(&tracker).as_ref());
        params.chunk_store = Some(store.clone_as_reader());
        params.ndc = Some(NamedDataCache::clone(&tracker));
        params.tracker = Some(TrackerCache::clone(&tracker));
        params.known_device = Some(vec![src_dev.clone()]);

        struct DownloadFromSource {
            src_dev: Device, 
            store: MemChunkStore,
            default: DefaultNdnEventHandler
        }

        #[async_trait::async_trait]
        impl NdnEventHandler for DownloadFromSource {
            async fn on_newly_interest(
                &self, 
                stack: &Stack, 
                interest: &Interest, 
                from: &Channel
            ) -> BuckyResult<()> {
                let _ = download_chunk(
                    stack,
                    interest.chunk.clone(), 
                    None, 
                    Some(SingleDownloadContext::desc_streams(None, vec![self.src_dev.desc().clone()])),
                    vec![self.store.clone_as_writer()],
                )
                .await;
                self.default.on_newly_interest(stack, interest, from).await
            }
        
            fn on_unknown_piece_data(
                &self, 
                _stack: &Stack, 
                _piece: &PieceData, 
                _from: &Channel
            ) -> BuckyResult<DownloadSession> {
                unimplemented!()
            }
        }
        params.ndn_event = Some(Box::new(DownloadFromSource {
            default: DefaultNdnEventHandler::new(), 
            src_dev: src_dev.clone(), 
            store: store.clone()
        }));
        (
            Stack::open(cache_dev, cache_secret, params).await.unwrap(),
            store,
        )
    };

    let (_src_stack, src_store) = {
        let mut params = StackOpenParams::new("bdt-example-upload-from-downloader-src");
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

    let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
    let chunk_hash = hash_data(&chunk_data[..]);
    let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);

    let to_put = Arc::new(chunk_data);
    let _ = src_store
        .add(chunkid.clone(), to_put.clone())
        .await
        .unwrap();

    let _ = download_chunk(
        &*down_stack,
        chunkid.clone(), 
        None, 
        Some(SingleDownloadContext::desc_streams(None, vec![cache_stack.local_const().clone()])),
        vec![down_store.clone_as_writer()],
    )
    .await;
    let recv = future::timeout(
        Duration::from_secs(50),
        watch_recv_chunk(down_stack.clone(), chunkid.clone()),
    )
    .await
    .unwrap();
    let recv_chunk_id = recv.unwrap();
    assert_eq!(recv_chunk_id, chunkid);

}
