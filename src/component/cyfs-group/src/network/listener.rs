use cyfs_base::{ObjectId, RawDecode};
use cyfs_bdt::DatagramTunnelGuard;

use crate::{GroupRPathMgr, HotstuffPackage};

pub struct Listener;

impl Listener {
    pub fn spawn(
        datagram: DatagramTunnelGuard,
        processor: GroupRPathMgr,
        local_device_id: ObjectId,
    ) {
        async_std::task::spawn(async move {
            Self::run(datagram, processor, local_device_id).await;
        });
    }

    async fn run(
        datagram: DatagramTunnelGuard,
        processor: GroupRPathMgr,
        local_device_id: ObjectId,
    ) {
        loop {
            match datagram.recv_v().await {
                Ok(pkgs) => {
                    for datagram in pkgs {
                        let remote = datagram.source.remote.object_id().clone();
                        match HotstuffPackage::raw_decode(datagram.data.as_slice()) {
                            Ok((pkg, remain)) => {
                                log::debug!(
                                    "[group-listener] {:?}-{} recv message from {:?}, msg: {:?}, len: {}",
                                    pkg.rpath(),
                                    local_device_id,
                                    remote,
                                    pkg,
                                    datagram.data.len()
                                );
                                assert_eq!(remain.len(), 0);
                                processor.on_message(pkg, remote).await;
                            }
                            Err(err) => {
                                log::debug!(
                                    "[group-listener] {} recv message from {:?}, len: {} decode failed {:?}",
                                    local_device_id,
                                    remote,
                                    datagram.data.len(),
                                    err
                                );
                            }
                        }
                    }
                }
                Err(e) => log::warn!("group listener failed: {:?}", e),
            }
        }
    }
}
