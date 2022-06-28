use crate::{
    protocol::{self, DynamicPackage, OnPackage, OnPackageResult},
    stack::{Stack, WeakStack},
    tunnel::{BuildTunnelParams, TunnelContainer, TunnelState},
    types::*,
};
use async_std::{pin::Pin, sync::Arc, task};
use cyfs_base::*;
use cyfs_debug::Mutex;
use futures::{
    task::{Context, Poll},
    Future,
};
use log::*;
use std::{
    collections::LinkedList,
    ops::{Deref, Drop},
    task::Waker,
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DatagramSource {
    pub remote: DeviceId,
    pub vport: u16,
}

#[derive(Clone)]
pub struct DatagramOptions {
    pub sequence: Option<TempSeq>,
    pub author_id: Option<DeviceId>,
    pub create_time: Option<Timestamp>,
    pub send_time: Option<Timestamp>,
    pub pieces: Option<u8>,
}

impl Default for DatagramOptions {
    fn default() -> Self {
        Self {
            sequence: None,
            author_id: None,
            create_time: None,
            send_time: None,
            pieces: None,
        }
    }
}

pub struct Datagram {
    pub source: DatagramSource,
    pub options: DatagramOptions,
    pub data: Vec<u8>,
}

struct RecvBuffer {
    capability: usize,
    waker: Option<Waker>,
    buffer: LinkedList<Datagram>,
}

struct DatagramTunnelImpl {
    stack: WeakStack,
    sequence: TempSeqGenerator,
    vport: u16,
    recv_buffer: Mutex<RecvBuffer>,
}

impl DatagramTunnelImpl {
    fn poll_recv_v(
        &self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<LinkedList<Datagram>, std::io::Error>> {
        let mut recv_buffer = self.recv_buffer.lock().unwrap();
        if recv_buffer.buffer.len() == 0 {
            // assert_eq!(recv_buffer.waker.is_none(), true);
            recv_buffer.waker = Some(cx.waker().clone());
            Poll::Pending
        } else {
            let mut datagrams = LinkedList::new();
            datagrams.append(&mut recv_buffer.buffer);
            Poll::Ready(Ok(datagrams))
        }
    }
}

impl std::fmt::Display for DatagramTunnelImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DatagramTunnel{{vport:{}}}", self.vport)
    }
}

#[derive(Clone)]
pub struct DatagramTunnel(Arc<DatagramTunnelImpl>);

impl AsRef<DatagramTunnelImpl> for DatagramTunnel {
    fn as_ref(&self) -> &DatagramTunnelImpl {
        &self.0
    }
}

impl DatagramTunnel {
    pub(crate) fn new(stack: WeakStack, vport: u16, recv_buffer: usize) -> DatagramTunnel {
        DatagramTunnel(Arc::new(DatagramTunnelImpl {
            stack,
            sequence: TempSeqGenerator::new(),
            vport,
            recv_buffer: Mutex::new(RecvBuffer {
                capability: recv_buffer,
                waker: None,
                buffer: LinkedList::new(),
            }),
        }))
    }

    pub fn recv_v(&self) -> impl Future<Output = Result<LinkedList<Datagram>, std::io::Error>> {
        RecvV {
            tunnel: self.clone(),
        }
    }

    pub fn measure_data(&self, _options: &DatagramOptions) -> BuckyResult<usize> {
        // let datagram = protocol::Datagram {
        //     to_vport: vport,
        //     from_vport: self.vport(),
        //     dest_zone: None,
        //     hop_limit: None,
        //     sequence: if options.with_sequence.is_some() {
        //         let seq = self.0.sequence.generate();
        //         options.with_sequence = Some(seq);
        //         Some(seq)
        //     } else {
        //         None
        //     },
        //     piece: None,
        //     send_time: if options.with_sendtime.is_some() {
        //         let sendtime = bucky_time_now();
        //         options.with_sendtime = Some(sendtime);
        //         Some(sendtime)
        //     } else {
        //         None
        //     },
        //     create_time: options.create_time,
        //     author_id: options.author_id,
        //     author: None,
        //     inner_type: protocol::DatagramType::Data,
        //     data: TailedOwnedData::from(vec![]),
        // };
        // let size = datagram.raw_measure(purpose)?;
        // Ok(interface::udp::MTU - KeyMixHash::raw_bytes().unwrap() - size)
        // FIXME: 正确的实现
        Ok(1024)
    }

    pub fn send_to_v(
        &self,
        _buf: &[&[u8]],
        _options: &DatagramOptions,
        _remote: &DeviceId,
        _vport: u16,
    ) -> Result<(), std::io::Error> {
        unimplemented!()
    }

    pub fn send_to(
        &self,
        buf: &[u8],
        options: &mut DatagramOptions,
        remote: &DeviceId,
        vport: u16,
    ) -> Result<(), std::io::Error> {
        assert_eq!(options.pieces.is_none(), true);
        let datagram = protocol::Datagram {
            to_vport: vport,
            from_vport: self.0.vport,
            dest_zone: None,
            hop_limit: None,
            sequence: if options.sequence.is_some() {
                let seq = options.sequence.unwrap();
                if seq == TempSeq::default() {
                    let seq = self.0.sequence.generate();
                    options.sequence = Some(seq);
                    Some(seq)
                } else {
                    Some(seq)
                }
            } else {
                None
            },
            piece: None,
            send_time: if options.send_time.is_some() {
                let sendtime = bucky_time_now();
                options.send_time = Some(sendtime);
                Some(sendtime)
            } else {
                None
            },
            create_time: options.create_time,
            author_id: options.author_id.as_ref().map(|id| id.clone()),
            author: None,
            inner_type: protocol::DatagramType::Data,
            data: TailedOwnedData::from(buf),
        };
        trace!(
            "{} try send {} to {}:{}",
            self.as_ref(),
            datagram,
            remote,
            vport
        );
        let stack = Stack::from(&self.as_ref().stack);
        let tunnel = stack.tunnel_manager().container_of(remote);
        if let Some(tunnel) = tunnel {
            if tunnel.state() == TunnelState::Dead {
                debug!(
                    "{} tunnel to {} dead, will build tunnel",
                    self.as_ref(),
                    remote
                );
                let arc_self = self.clone();
                let remote = remote.to_owned();
                task::spawn(async move {
                    if let Some(remote_device) = stack.device_cache().get(&remote).await {
                        let build_params = BuildTunnelParams {
                            remote_const: remote_device.desc().clone(),
                            remote_sn: stack.sn_client().sn_list(),
                            remote_desc: Some(remote_device),
                        };
                        let _ = tunnel.build_send(DynamicPackage::from(datagram), build_params);
                    } else {
                        warn!(
                            "{} build tunnel to {} failed for device not in cache",
                            arc_self.as_ref(),
                            remote
                        );
                    }
                });
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "pending on building tunnel",
                ))
            } else {
                tunnel
                    .send_package(DynamicPackage::from(datagram))
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.msg()))
            }
        } else {
            debug!(
                "{} tunnel to {} not exists, will build tunnel",
                self.as_ref(),
                remote
            );
            let arc_self = self.clone();
            let remote = remote.to_owned();
            task::spawn(async move {
                if let Some(remote_device) = stack.device_cache().get(&remote).await {
                    let tunnel = stack
                        .tunnel_manager()
                        .create_container(remote_device.desc())
                        .unwrap();
                    let build_params = BuildTunnelParams {
                        remote_const: remote_device.desc().clone(),
                        remote_sn: stack.sn_client().sn_list(),
                        remote_desc: Some(remote_device),
                    };
                    let _ = tunnel.build_send(DynamicPackage::from(datagram), build_params);
                } else {
                    warn!(
                        "{} build tunnel to {} failed for device not in cache",
                        arc_self.as_ref(),
                        remote
                    );
                }
            });
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "pending on building tunnel",
            ))
        }
    }

    pub fn vport(&self) -> u16 {
        self.0.vport
    }

    pub fn close(&self) {
        let stack = Stack::from(&self.0.stack);
        stack.datagram_manager().unbind(self.vport());
    }
}

// FIXME: 整个 OnPackage体系的 package参数改成转移不是引用，这里就可以不用拷贝data
impl OnPackage<protocol::Datagram, &TunnelContainer> for DatagramTunnel {
    fn on_package(
        &self,
        pkg: &protocol::Datagram,
        from: &TunnelContainer,
    ) -> Result<OnPackageResult, BuckyError> {
        log::trace!("{} recv {} from {}", self.as_ref(), pkg, from);
        assert_eq!(pkg.to_vport, self.vport());
        let datagram = Datagram {
            source: DatagramSource {
                remote: from.remote().clone(),
                vport: pkg.from_vport,
            },
            options: DatagramOptions {
                sequence: pkg.sequence,
                author_id: pkg.author_id.as_ref().map(|id| id.clone()),
                create_time: pkg.create_time,
                send_time: pkg.send_time,
                pieces: None,
            },
            data: Vec::from(pkg.data.as_ref()),
        };

        if let Some(waker) = {
            let mut recv_buffer = self.0.recv_buffer.lock().unwrap();
            if recv_buffer.buffer.len() == recv_buffer.capability {
                let _ = recv_buffer.buffer.pop_front();
            }
            recv_buffer.buffer.push_back(datagram);
            if let Some(ref waker) = recv_buffer.waker {
                let waker = waker.clone();
                recv_buffer.waker = None;
                Some(waker)
            } else {
                None
            }
        } {
            waker.wake();
        }
        Ok(OnPackageResult::Handled)
    }
}

pub struct RecvV {
    tunnel: DatagramTunnel,
}

impl Future for RecvV {
    type Output = Result<LinkedList<Datagram>, std::io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let tunnel = self.tunnel.clone();
        tunnel.0.poll_recv_v(cx)
    }
}

struct DatagramTunnelGuardImpl(DatagramTunnel);

impl Drop for DatagramTunnelGuardImpl {
    fn drop(&mut self) {
        self.0.close();
    }
}

#[derive(Clone)]
pub struct DatagramTunnelGuard(Arc<DatagramTunnelGuardImpl>);

impl From<DatagramTunnel> for DatagramTunnelGuard {
    fn from(tunnel: DatagramTunnel) -> Self {
        Self(Arc::new(DatagramTunnelGuardImpl(tunnel)))
    }
}

impl Deref for DatagramTunnelGuard {
    type Target = DatagramTunnel;
    fn deref(&self) -> &DatagramTunnel {
        &(*self.0).0
    }
}
