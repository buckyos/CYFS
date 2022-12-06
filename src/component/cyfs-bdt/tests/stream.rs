use std::{
    net::Shutdown, 
    time::Duration, 
};
use async_std::{
    task, 
    future, 
    channel, 
    io::prelude::{ReadExt, WriteExt}
};
use futures::StreamExt;
use cyfs_base::*;
use cyfs_util::cache::{NamedDataCache, TrackerCache};
use cyfs_bdt::*;
mod utils;

async fn recv_large_stream(stack: StackGuard) -> BuckyResult<Vec<u8>> {
    let acceptor = stack.stream_manager().listen(0).unwrap();
    let mut incoming = acceptor.incoming();
    let mut pre_stream = incoming.next().await.unwrap()?;
    pre_stream.stream.confirm(vec![].as_ref()).await?;
    let mut buffer = vec![];
    let _ = pre_stream.stream.read_to_end(&mut buffer).await?; 
    let _ = pre_stream.stream.shutdown(Shutdown::Both);
    Ok(buffer)
}

async fn send_large_stream(
    ln_stack: &StackGuard, 
    rn_dev: Device, 
    data: &[u8]) -> BuckyResult<()> {
    let param = BuildTunnelParams {
        remote_const: rn_dev.desc().clone(),
        remote_sn: vec![],
        remote_desc: Some(rn_dev.clone()),
    };
    let mut stream = ln_stack.stream_manager().connect(0u16, vec![], param).await?;
    stream.write_all(data).await?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

async fn large_stream(ln_ep: &[&str], rn_ep: &[&str]) {
    let ((ln_stack, _), (rn_stack, _)) = utils::local_stack_pair(
        ln_ep, 
        rn_ep).await.unwrap();
    let (sample_size, sample) = utils::random_mem(1024, 512);
    let (signal_sender, signal_recver) = channel::bounded::<BuckyResult<Vec<u8>>>(1);

    {
        let rn_stack = rn_stack.clone();
        task::spawn(async move {
            signal_sender.send(recv_large_stream(rn_stack).await).await.unwrap();
        });
    }
    send_large_stream(&ln_stack, rn_stack.sn_client().ping().default_local_device(), sample.as_ref()).await.unwrap();
    let recv = future::timeout(Duration::from_secs(5), signal_recver.recv()).await.unwrap().unwrap();
    let recv_sample = recv.unwrap();

    assert_eq!(recv_sample.len(), sample_size);
    let sample_hash = hash_data(sample.as_ref());
    let recv_hash = hash_data(recv_sample.as_ref());

    assert_eq!(sample_hash, recv_hash);
}

#[async_std::test]
async fn large_udp_stream() {
    large_stream(
        &["W4udp127.0.0.1:10000"], 
        &["W4udp127.0.0.1:10001"]).await
}

#[async_std::test]
async fn large_udp_stream_with_loss() {
     let mut uploader_config = StackConfig::new("");
    uploader_config.interface.udp.sim_loss_rate = 10;
    let ((ln_stack, _), (rn_stack, _)) = utils::local_stack_pair_with_config(
        &["W4udp127.0.0.1:10002"], 
        &["W4udp127.0.0.1:10003"], 
        Some(uploader_config), None).await.unwrap();
    let (sample_size, sample) = utils::random_mem(1024, 512);
    let (signal_sender, signal_recver) = channel::bounded::<BuckyResult<Vec<u8>>>(1);

    {
        let rn_stack = rn_stack.clone();
        task::spawn(async move {
            signal_sender.send(recv_large_stream(rn_stack).await).await.unwrap();
        });
    }
    send_large_stream(&ln_stack, rn_stack.sn_client().ping().default_local_device(), sample.as_ref()).await.unwrap();
    let recv = future::timeout(Duration::from_secs(5), signal_recver.recv()).await.unwrap().unwrap();
    let recv_sample = recv.unwrap();

    assert_eq!(recv_sample.len(), sample_size);
    let sample_hash = hash_data(sample.as_ref());
    let recv_hash = hash_data(recv_sample.as_ref());

    assert_eq!(sample_hash, recv_hash);
}


#[async_std::test]
async fn large_tcp_stream() {
    large_stream(
        &["W4tcp127.0.0.1:10000"], 
        &["W4tcp127.0.0.1:10001"]).await
}


#[async_std::test]
async fn error_tcp_endpoint() {
    let (ln_dev, ln_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4tcp127.0.0.1:10002"]).unwrap();
    let (rn_dev1, rn_secret1) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4tcp127.0.0.1:10003"]).unwrap();
    let (rn_dev2, rn_secret2) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4tcp127.0.0.1:10004"]).unwrap();

    let mut ln_params = StackOpenParams::new("");
    let ln_tracker = MemTracker::new();
    let ln_store = MemChunkStore::new(NamedDataCache::clone(&ln_tracker).as_ref());
    ln_params.chunk_store = Some(ln_store.clone_as_reader());
    ln_params.ndc = Some(NamedDataCache::clone(&ln_tracker));
    ln_params.tracker = Some(TrackerCache::clone(&ln_tracker));

    let mut rn_dev = rn_dev1.clone();

    let mut rn_endpoints = vec![rn_dev.connect_info().endpoints()[0].clone(), rn_dev2.connect_info().endpoints()[0].clone()];
    std::mem::swap(rn_dev.mut_connect_info().mut_endpoints(), &mut rn_endpoints);
    ln_params.known_device = Some(vec![rn_dev.clone()]);
    let ln_stack = Stack::open(ln_dev.clone(), ln_secret, ln_params).await.unwrap();


    let mut rn_params = StackOpenParams::new("");
    let rn_tracker = MemTracker::new();
    let rn_store = MemChunkStore::new(NamedDataCache::clone(&rn_tracker).as_ref());
    rn_params.chunk_store = Some(rn_store.clone_as_reader());
    rn_params.ndc = Some(NamedDataCache::clone(&rn_tracker));
    rn_params.tracker = Some(TrackerCache::clone(&rn_tracker));
   
    let rn_stack1 = Stack::open(rn_dev1, rn_secret1, rn_params).await.unwrap();


    let mut rn_params = StackOpenParams::new("");
    let rn_tracker = MemTracker::new();
    let rn_store = MemChunkStore::new(NamedDataCache::clone(&rn_tracker).as_ref());
    rn_params.chunk_store = Some(rn_store.clone_as_reader());
    rn_params.ndc = Some(NamedDataCache::clone(&rn_tracker));
    rn_params.tracker = Some(TrackerCache::clone(&rn_tracker));
    let _ = Stack::open(rn_dev2, rn_secret2, rn_params).await.unwrap();



    let (sample_size, sample) = utils::random_mem(1024, 512);
    let (signal_sender, signal_recver) = channel::bounded::<BuckyResult<Vec<u8>>>(1);

    {
        let rn_stack1 = rn_stack1.clone();
        task::spawn(async move {
            signal_sender.send(recv_large_stream(rn_stack1).await).await.unwrap();
        });
    }
    send_large_stream(&ln_stack, rn_dev, sample.as_ref()).await.unwrap();
    let recv = future::timeout(Duration::from_secs(5), signal_recver.recv()).await.unwrap().unwrap();
    let recv_sample = recv.unwrap();

    assert_eq!(recv_sample.len(), sample_size);
    let sample_hash = hash_data(sample.as_ref());
    let recv_hash = hash_data(recv_sample.as_ref());

    assert_eq!(sample_hash, recv_hash);
}