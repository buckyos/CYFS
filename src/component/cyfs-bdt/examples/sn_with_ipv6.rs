use async_std::{
    channel, future,
    io::prelude::{ReadExt, WriteExt},
    task,
};
use cyfs_base::*;
use futures::StreamExt;
use cyfs_bdt::{
    *, 
    sn::service::*
};
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

struct TestServer {

}


impl SnServiceContractServer for TestServer {
    fn check_receipt(
        &self,
        _client_device: &Device,
        _local_receipt: &SnServiceReceipt,
        _client_receipt: &Option<ReceiptWithSignature>,
        _last_request_time: &ReceiptRequestTime,
    ) -> IsAcceptClient {
        IsAcceptClient::Accept(false)
    }

    fn verify_auth(&self, _client_device_id: &DeviceId) -> IsAcceptClient {
        IsAcceptClient::Accept(false)
    }
}


#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example-sn");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-sn")
        .level("trace")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-sn", "bdt-example-sn")
        .exit_on_panic(true)
        .build()
        .start();

    let ipv6_ep = format!("D6udp[{:?}]:10024", &local_ip_address::list_afinet_netifas().unwrap()[0].1);

    let (sn, sn_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["L4udp127.0.0.1:10024", &ipv6_ep]).unwrap();

    let service = SnService::new(
        sn.clone(),
        sn_secret,
        Box::new(TestServer {}),
    );

    task::spawn(async move {
        let _ = service.start().await;
    });
    

    let (ln_dev, ln_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["L4udp127.0.0.1:10025", "D6udp[::]:10025", "D6tcp[::]:10025"]).unwrap();
    let (rn_dev, rn_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["L4udp127.0.0.1:10026", "D6udp[::]:10026", "D6tcp[::]:10026"]).unwrap();
    
    let mut ln_params = StackOpenParams::new("");
    ln_params.known_device = Some(vec![rn_dev.clone(), sn.clone()]);
    ln_params.config.interface.udp.sn_only = true;

    let ln_stack = Stack::open(
        ln_dev.clone(), 
        ln_secret, 
        ln_params).await.unwrap();


    let mut rn_params = StackOpenParams::new("");
    rn_params.known_sn = Some(vec![sn.clone()]);
    let rn_stack = Stack::open(
        rn_dev, 
        rn_secret, 
        rn_params).await.unwrap();
    
    assert_eq!(SnStatus::Online, rn_stack.sn_client().ping().wait_online().await.unwrap());

    let (sample_size, sample) = utils::random_mem(1024, 512);
    let (signal_sender, signal_recver) = channel::bounded::<BuckyResult<Vec<u8>>>(1);
    {
        let rn_stack = rn_stack.clone();
        task::spawn(async move {
            signal_sender.send(recv_large_stream(rn_stack).await).await.unwrap();
        });
    }

    let param = BuildTunnelParams {
        remote_const: rn_stack.local_const().clone(),
        remote_sn: Some(vec![sn.desc().device_id()]),
        remote_desc: None,
    };
    let mut stream = ln_stack
        .stream_manager()
        .connect(0u16, vec![], param)
        .await.unwrap();
    stream.write_all(&sample[..]).await.unwrap();

    let _ = stream.shutdown(Shutdown::Both);

    let recv = future::timeout(Duration::from_secs(5), signal_recver.recv()).await.unwrap().unwrap();
    let recv_sample = recv.unwrap();

    assert_eq!(recv_sample.len(), sample_size);
    let sample_hash = hash_data(sample.as_ref());
    let recv_hash = hash_data(recv_sample.as_ref());

    assert_eq!(sample_hash, recv_hash);

}
