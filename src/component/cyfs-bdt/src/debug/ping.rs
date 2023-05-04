use cyfs_base::*;
use crate::{
    stack::{
        WeakStack, 
        Stack
    }, 
    datagram::{
        self, 
        DatagramOptions, 
        DatagramTunnelGuard
    }, 
    types::*,
};
use async_std::{
    sync::Arc, 
    task, 
    future, 
};
use std::time::Duration;
use std::io::ErrorKind;

struct PingStubImpl {
    stack: WeakStack,
}

#[derive(Clone)]
pub struct PingStub(Arc<PingStubImpl>);

impl PingStub {
    pub fn new(weak_stack: WeakStack) -> Self {
        Self(Arc::new(PingStubImpl {
            stack: weak_stack, 
        }))
    }

    pub fn listen(&self) {
        let stack = Stack::from(&self.0.stack);
        let tunnel = stack.datagram_manager().bind_reserved(datagram::ReservedVPort::Debug).unwrap();
        task::spawn(async move {
            loop {
                match tunnel.recv_v().await {
                    Ok(datagrams) => {
                        for datagram in datagrams {
                            let mut options = datagram.options.clone();
                            let len = datagram.data.len();
                            if let Err(err) = tunnel.send_to(
                                datagram.data.as_ref(),
                                &mut options, 
                                &datagram.source.remote, 
                                datagram.source.vport) {
                                error!("ping from remote={:?} vport={:?} len={} resp err={:?}", 
                                    datagram.source.remote, datagram.source.vport, len, err);
                            } else {
                                debug!("ping from remote={:?} vport={:?} len={}", 
                                    datagram.source.remote, datagram.source.vport, len);
                            }
                        }
                    }, 
                    Err(err) => {
                        error!("ping recv err={:?}", err);
                    }
                }
            }
        });
    }

    pub fn ping(&self) -> BuckyResult<u64> {
        let t = bucky_time_now();

        Ok(bucky_time_now() - t)
    }
}

struct PingerImpl {
    datagram_tunnel: DatagramTunnelGuard,
}

#[derive(Clone)]
pub struct Pinger(Arc<PingerImpl>);

impl Pinger {
    pub fn open(weak_stack: WeakStack) -> BuckyResult<Self> {
        let stack = Stack::from(&weak_stack);
        let datagram_tunnel = stack.datagram_manager().bind(0)
            .map_err(|err| format!("bind datagram tunnel failed for {}", err))?;

        Ok(Self(Arc::new(PingerImpl {
            datagram_tunnel, 
        })))
    }

    pub async fn ping(&self, remote: Device, timeout: Duration, buf: &[u8]) -> BuckyResult<Option<u64>> { //us
        let mut options = DatagramOptions::default();

        let ts = cyfs_base::bucky_time_now();
        options.sequence = Some(TempSeq::from(ts as u32));

        if let Err(err) = self.0.datagram_tunnel.send_to(
            buf, 
            &mut options, 
            &remote.desc().device_id(), 
            datagram::ReservedVPort::Debug.into()) {
            match err.kind() {
                ErrorKind::NotConnected => {
                },
                _ => {
                    return Err(BuckyError::new(BuckyErrorCode::CodeError, format!("ping remote={:?} send err={:?}", remote, err)));
                }
            }
        }

         match future::timeout(timeout, self.0.datagram_tunnel.recv_v()).await {
            Err(err) => {
                return Err(BuckyError::new(BuckyErrorCode::CodeError, format!("ping remote={:?} wait err={:?}", remote, err)))
            },
            Ok(res) => {
                let cost = cyfs_base::bucky_time_now() - ts;
                match res {
                    Err(err) => {
                        return Err(BuckyError::new(BuckyErrorCode::CodeError, format!("ping remote={:?} err={:?}", remote, err)))
                    },
                    Ok(datagrams) => {
                        for datagram in datagrams {
                            if let Some(opt) = datagram.options.sequence {
                                if opt == options.sequence.unwrap() {

                                    return Ok(Some(cost))
                                }
                            }
                        }

                        return Ok(None)
                    }
                }
            }
        }
    }
}
