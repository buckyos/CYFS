use async_std::{
    channel, future,
    io::prelude::{ReadExt, WriteExt},
    task,
};
use cyfs_base::*;
use futures::StreamExt;
use cyfs_bdt::{BuildTunnelParams, StackConfig, StackGuard};
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
    rn_stack: &StackGuard,
    data: &[u8],
) -> BuckyResult<()> {
    let param = BuildTunnelParams {
        remote_const: rn_stack.local_const().clone(),
        remote_sn: vec![],
        remote_desc: Some(rn_stack.local().clone()),
    };
    let mut stream = ln_stack
        .stream_manager()
        .connect(0u16, vec![], param)
        .await?;
    stream.write_all(data).await?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

async fn large_stream(ln_ep: &[&str], rn_ep: &[&str]) {
    let ((ln_stack, _), (rn_stack, _)) = utils::local_stack_pair(ln_ep, rn_ep).await.unwrap();
    let (sample_size, sample) = utils::random_mem(1024, 512);
    let (signal_sender, signal_recver) = channel::bounded::<BuckyResult<Vec<u8>>>(1);

    {
        let rn_stack = rn_stack.clone();
        task::spawn(async move {
            signal_sender
                .send(recv_large_stream(rn_stack).await)
                .await
                .unwrap();
        });
    }
    send_large_stream(&ln_stack, &rn_stack, sample.as_ref())
        .await
        .unwrap();
    let recv = future::timeout(Duration::from_secs(5), signal_recver.recv())
        .await
        .unwrap()
        .unwrap();
    let recv_sample = recv.unwrap();

    assert_eq!(recv_sample.len(), sample_size);
    let sample_hash = hash_data(sample.as_ref());
    let recv_hash = hash_data(recv_sample.as_ref());

    assert_eq!(sample_hash, recv_hash);
}

async fn large_udp_stream() {
    large_stream(&["W4udp127.0.0.1:10000"], &["W4udp127.0.0.1:10001"]).await
}

async fn large_udp_stream_with_loss() {
    let mut uploader_config = StackConfig::new("");
    uploader_config.interface.udp.sim_loss_rate = 10;
    let ((ln_stack, _), (rn_stack, _)) = utils::local_stack_pair_with_config(
        &["W4udp127.0.0.1:10002"],
        &["W4udp127.0.0.1:10003"],
        Some(uploader_config),
        None,
    )
    .await
    .unwrap();
    let (sample_size, sample) = utils::random_mem(1024, 512);
    let (signal_sender, signal_recver) = channel::bounded::<BuckyResult<Vec<u8>>>(1);

    {
        let rn_stack = rn_stack.clone();
        task::spawn(async move {
            signal_sender
                .send(recv_large_stream(rn_stack).await)
                .await
                .unwrap();
        });
    }
    send_large_stream(&ln_stack, &rn_stack, sample.as_ref())
        .await
        .unwrap();
    let recv = future::timeout(Duration::from_secs(5), signal_recver.recv())
        .await
        .unwrap()
        .unwrap();
    let recv_sample = recv.unwrap();

    assert_eq!(recv_sample.len(), sample_size);
    let sample_hash = hash_data(sample.as_ref());
    let recv_hash = hash_data(recv_sample.as_ref());

    assert_eq!(sample_hash, recv_hash);
}

async fn large_tcp_stream() {
    large_stream(&["W4tcp127.0.0.1:10000"], &["W4tcp127.0.0.1:10001"]).await
}

#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example-stream");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-stream")
        .level("trace")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-stream", "bdt-example-stream")
        .exit_on_panic(true)
        .build()
        .start();

    large_udp_stream().await;

    task::sleep(Duration::from_secs(10000000000)).await;
}
