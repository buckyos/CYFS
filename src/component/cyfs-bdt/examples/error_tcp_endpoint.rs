use async_std::{
    channel, future,
    io::prelude::{ReadExt, WriteExt},
    task,
};
use cyfs_base::*;
use futures::StreamExt;
use cyfs_bdt::*;
use std::{net::Shutdown, time::Duration};
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
    data: &[u8],
) -> BuckyResult<()> {
    let param = BuildTunnelParams {
        remote_const: rn_dev.desc().clone(),
        remote_sn: vec![],
        remote_desc: Some(rn_dev),
    };
    let mut stream = ln_stack
        .stream_manager()
        .connect(0u16, vec![], param)
        .await?;
    stream.write_all(data).await?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}


#[async_std::main]
async fn main() {

    cyfs_util::process::check_cmd_and_exec("bdt-example-error-tcp-endpoints");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-error-tcp-endpoints")
        .level("trace")
        .console("debug")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-error-tcp-endpoints", "bdt-example-error-tcp-endpoints")
        .exit_on_panic(true)
        .build()
        .start();

    let (ln_dev, ln_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4tcp127.0.0.1:10002"]).unwrap();
    let (rn_dev1, rn_secret1) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4tcp127.0.0.1:10003"]).unwrap();
    let (rn_dev2, rn_secret2) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4tcp127.0.0.1:10004"]).unwrap();

    let mut ln_params = StackOpenParams::new("");
    let ln_store = MemChunkStore::new();
    ln_params.chunk_store = Some(ln_store.clone_as_reader());

    let mut rn_dev = rn_dev1.clone();

    let mut rn_endpoints = vec![rn_dev2.connect_info().endpoints()[0].clone(), rn_dev1.connect_info().endpoints()[0].clone()];
    std::mem::swap(rn_dev.mut_connect_info().mut_endpoints(), &mut rn_endpoints);
    ln_params.known_device = Some(vec![rn_dev.clone()]);
    let ln_stack = Stack::open(ln_dev.clone(), ln_secret, ln_params).await.unwrap();


    let mut rn_params = StackOpenParams::new("");
    let rn_store = MemChunkStore::new();
    rn_params.chunk_store = Some(rn_store.clone_as_reader()); 
    let rn_stack1 = Stack::open(rn_dev1, rn_secret1, rn_params).await.unwrap();


    let mut rn_params = StackOpenParams::new("");
    let rn_store = MemChunkStore::new();
    rn_params.chunk_store = Some(rn_store.clone_as_reader());
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