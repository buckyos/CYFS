use async_trait::async_trait;
use cyfs_base::{BuckyError, DeviceId};
use cyfs_bdt::{DatagramTunnelGuard as DatagramTunnel, DatagramOptions};
use std::str::FromStr;

#[async_trait]
pub trait UpstreamDatagramSender: Sync + Send {
    // add code here

    async fn send_to(&self, buf: &[u8], src_addr: &str) -> Result<usize, BuckyError>;
    fn clone_sender(&self) -> Box<dyn UpstreamDatagramSender>;
}

use async_std::net::UdpSocket;
use std::sync::Arc;

#[async_trait]
impl UpstreamDatagramSender for Arc<UdpSocket> {
    async fn send_to(&self, buf: &[u8], src_addr: &str) -> Result<usize, BuckyError> {
        debug!("will reply to udp sock: {}", src_addr);
        UdpSocket::send_to(self, buf, src_addr)
            .await
            .map_err(|e| BuckyError::from(e))
    }

    fn clone_sender(&self) -> Box<dyn UpstreamDatagramSender> {
        Box::new(self.clone()) as Box<dyn UpstreamDatagramSender>
    }
}

#[async_trait]
impl UpstreamDatagramSender for DatagramTunnel {
    async fn send_to(&self, buf: &[u8], src_addr: &str) -> Result<usize, BuckyError> {
        let addr: Vec<&str> = src_addr.split(":").collect();
        let device_id = DeviceId::from_str(addr[0]).unwrap();
        let vport = addr[1].parse::<u16>().unwrap();

        let mut options = DatagramOptions {
            sequence: None,
            author_id: None,
            create_time: None,
            send_time: None,
            pieces: None
        };
        (**self).send_to(buf, &mut options, &device_id, vport).map_err(|e| BuckyError::from(e))?;

        Ok(buf.len())
    }

    fn clone_sender(&self) -> Box<dyn UpstreamDatagramSender> {
        Box::new(self.clone()) as Box<dyn UpstreamDatagramSender>
    }
}
