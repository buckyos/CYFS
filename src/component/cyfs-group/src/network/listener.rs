use cyfs_base::RawDecode;
use cyfs_bdt::DatagramTunnelGuard;

use crate::{GroupRPathMgr, HotstuffPackage};

pub struct Listener;

impl Listener {
    pub fn spawn(datagram: DatagramTunnelGuard, processor: GroupRPathMgr) {
        async_std::task::spawn(async move {
            Self::run(datagram, processor).await;
        });
    }

    async fn run(datagram: DatagramTunnelGuard, processor: GroupRPathMgr) {
        loop {
            match datagram.recv_v().await {
                Ok(pkgs) => {
                    for pkg in pkgs {
                        let remote = pkg.source.remote.object_id().clone();
                        if let Ok((pkg, remain)) = HotstuffPackage::raw_decode(pkg.data.as_slice())
                        {
                            assert_eq!(remain.len(), 0);
                            processor.on_message(pkg, remote).await;
                        }
                    }
                }
                Err(e) => log::warn!("group listener failed: {:?}", e),
            }
        }
    }
}
