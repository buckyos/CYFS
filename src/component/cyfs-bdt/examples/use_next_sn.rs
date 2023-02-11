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


async fn recv_large_stream(stack: StackGuard, sender: channel::Sender<Vec<u8>>) {
    let acceptor = stack.stream_manager().listen(0).unwrap();
    let mut incoming = acceptor.incoming();
    loop {
        let mut pre_stream = incoming.next().await.unwrap().unwrap();
        pre_stream.stream.confirm(vec![].as_ref()).await.unwrap();
        let mut buffer = vec![];
        let _ = pre_stream.stream.read_to_end(&mut buffer).await.unwrap();
        let _ = pre_stream.stream.shutdown(Shutdown::Both);
        sender.send(buffer).await.unwrap();
    }  
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

        let (sn1, sn1_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4udp127.0.0.1:10020"]).unwrap();

        let (sn2, sn2_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4udp127.0.0.1:10021"]).unwrap();
    
    
        let (ln_dev, ln_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["L4udp127.0.0.1:10022"]).unwrap();
        let (rn_dev, rn_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["L4udp127.0.0.1:10023"]).unwrap();
    
        let (sn_dev, sn_secret) = {
            let rn_id = rn_dev.desc().object_id();
            let sn1_id = sn1.desc().object_id();
            let sn2_id = sn1.desc().object_id();
    
            if rn_id.distance_of(&sn1_id) > rn_id.distance_of(&sn2_id) {
                (sn1.clone(), sn1_secret)
            } else {
                (sn2.clone(), sn2_secret)
            }
        };
    
    
        let service = SnService::new(
            sn_dev,
            sn_secret,
            Box::new(TestServer {}),
        );
    
        task::spawn(async move {
            let _ = service.start().await;
        });
    
    
        let mut ln_params = StackOpenParams::new("");
        ln_params.known_device = Some(vec![rn_dev.clone(), sn1.clone(), sn2.clone()]);
        let ln_stack = Stack::open(
            ln_dev.clone(), 
            ln_secret, 
            ln_params).await.unwrap();
    
    
        let mut rn_params = StackOpenParams::new("");
        rn_params.config.sn_client.ping.udp.resend_timeout = Duration::from_secs(1);
        rn_params.known_sn = Some(vec![sn1.clone(), sn2.clone()]);
        let rn_stack = Stack::open(
            rn_dev, 
            rn_secret, 
            rn_params).await.unwrap();
    
        assert_eq!(SnStatus::Online, rn_stack.sn_client().ping().wait_online().await.unwrap());
    
        let (sample_size, sample) = utils::random_mem(1024, 512);
        let (signal_sender, signal_recver) = channel::bounded::<Vec<u8>>(1);
        {
            let rn_stack = rn_stack.clone();
            task::spawn(async move {
                recv_large_stream(rn_stack, signal_sender).await;
            });
        }
    
        let param = BuildTunnelParams {
            remote_const: rn_stack.local_const().clone(),
            remote_sn: Some(vec![sn1.desc().device_id(), sn2.desc().device_id()]),
            remote_desc: None,
        };
        let mut stream = ln_stack
            .stream_manager()
            .connect(0u16, vec![], param)
            .await.unwrap();
        stream.write_all(&sample[..]).await.unwrap();
    
        let _ = stream.shutdown(Shutdown::Both);
    
        let recv_sample = future::timeout(Duration::from_secs(5), signal_recver.recv()).await.unwrap().unwrap();
    
        assert_eq!(recv_sample.len(), sample_size);
        let sample_hash = hash_data(sample.as_ref());
        let recv_hash = hash_data(recv_sample.as_ref());
    
        assert_eq!(sample_hash, recv_hash);

}


// #[async_std::main]
// async fn main() {
// }