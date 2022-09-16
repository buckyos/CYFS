use std::{
    time::Duration, 
};
use async_std::{
    task, 
    future, 
};
use cyfs_base::*;
use cyfs_bdt::{Datagram, DatagramTunnelGuard, DatagramOptions};
mod utils;




async fn watch_answer(tunnel: DatagramTunnelGuard) -> BuckyResult<Datagram> {
    let mut datagrams = tunnel.recv_v().await?;
    if datagrams.len() != 1 {
        return Err(BuckyError::new(BuckyErrorCode::InvalidData, "multi answer"));
    }
    let datagram = datagrams.pop_front().unwrap();
    Ok(datagram)
}


async fn send_util_ok(
    tunnel: &DatagramTunnelGuard, 
    data: &[u8], 
    options: &mut DatagramOptions, 
    to: &DeviceId, 
    port: u16, 
    interval: Duration) -> std::io::Result<()> {
    loop {
        match tunnel.send_to(
            data, 
            options, 
            to, 
            port) {
            Ok(_) => break Ok(()), 
            Err(err) => {
                match err.kind() {
                    std::io::ErrorKind::NotConnected => {
                        task::sleep(interval).await;
                        continue;
                    }, 
                    _ => break Err(err)
                }
            }
        }
    }
}

async fn send_with_timeout(
    tunnel: &DatagramTunnelGuard, 
    data: &[u8], 
    options: &mut DatagramOptions, 
    to: &DeviceId, 
    port: u16, 
    interval: Duration, 
    timeout: Duration) -> std::io::Result<()> {
    
    match future::timeout(timeout, send_util_ok(tunnel, data, options, to, port, interval)).await {
        Ok(r) => r, 
        Err(_) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"))
    }
}


async fn watch_question(tunnel: DatagramTunnelGuard, answer: Vec<u8>) -> BuckyResult<Datagram> {
    let mut datagrams = tunnel.recv_v().await?;
    if datagrams.len() != 1 {
        return Err(BuckyError::new(BuckyErrorCode::InvalidData, "multi question"));
    }
    let datagram = datagrams.pop_front().unwrap();
    let mut options = datagram.options.clone();
    let _ = tunnel.send_to(answer.as_ref(), &mut options, &datagram.source.remote, datagram.source.vport)?;
    Ok(datagram)
}

async fn datagram_qa(ln_ep: &[&str], rn_ep: &[&str]) {
    let ((ln_stack, _), (rn_stack, _)) = utils::local_stack_pair(
        ln_ep, 
        rn_ep).await.unwrap();
    
    let qa_port = 10000;
    let question = b"question".to_vec();
    let answer = b"answer".to_vec();
    let question_tunnel = ln_stack.datagram_manager().bind(qa_port).unwrap();
    let answer_tunnel = rn_stack.datagram_manager().bind(qa_port).unwrap();
    let mut question_options = DatagramOptions::default();
    let _ = send_with_timeout(
        &question_tunnel, 
        question.as_ref(), 
        &mut question_options, 
        rn_stack.local_device_id(), 
        qa_port, 
        Duration::from_millis(500), 
        Duration::from_secs(5)).await.unwrap();
    
    
    {
        let answer_tunnel = answer_tunnel.clone();
        let answer = answer.clone();
        let ln_stack = ln_stack.clone();
        task::spawn(async move {
            let recv_question = watch_question(answer_tunnel, answer).await.unwrap();
            assert!(recv_question.source.remote.eq(ln_stack.local_device_id()));
            assert_eq!(recv_question.source.vport, qa_port);
        });
    }

    let recv = future::timeout(Duration::from_secs(5), watch_answer(question_tunnel)).await.unwrap();
    let recv_answer = recv.unwrap();
    assert!(recv_answer.source.remote.eq(rn_stack.local_device_id()));
    assert_eq!(recv_answer.source.vport, qa_port);
}

#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example-datagram");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-datagram")
        .level("trace")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-datagram", "bdt-example-datagram")
        .exit_on_panic(true)
        .build()
        .start();

    datagram_qa(
        &["W4udp127.0.0.1:10000"], 
        &["W4udp127.0.0.1:10001"]).await
}


