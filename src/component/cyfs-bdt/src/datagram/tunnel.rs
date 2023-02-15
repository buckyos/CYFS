use crate::{
    protocol::{self, *},
    stack::{Stack, WeakStack},
    tunnel::{BuildTunnelParams, TunnelContainer, TunnelState},
    types::*, 
    MTU
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
    collections::{LinkedList, HashMap},
    ops::{Deref, Drop},
    task::Waker,
    time::Duration,
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
    pub plaintext: bool,
}

impl Default for DatagramOptions {
    fn default() -> Self {
        Self {
            sequence: None,
            author_id: None,
            create_time: None,
            send_time: None,
            plaintext: false,
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

struct DatagramFragment {
    author_id: DeviceId,
    from_vport: u16,
    sequence: TempSeq,
    to_vport: u16,
	expire_time: u64,
	datagrams: HashMap<u8, protocol::v0::Datagram>,
	fragment_total: usize,
}

struct DatagramFragments {
    fragments: HashMap<String, DatagramFragment>,
    frag_data_size: usize,
    frag_data_max_size: usize,
    frag_expired_us: u64,
}

struct DatagramTunnelImpl {
    stack: WeakStack,
    sequence: TempSeqGenerator,
    vport: u16,
    recv_buffer: Mutex<RecvBuffer>,
    frag_buffer: Arc<Mutex<DatagramFragments>>,
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
        let cfg = Stack::from(&stack).config().datagram.clone();
        let expired_tick_sec = cfg.expired_tick_sec;
        let fragment_cache_size = cfg.fragment_cache_size;
        let fragment_expired_us = cfg.fragment_expired_us;

        let datagram_tunnel = DatagramTunnel(Arc::new(DatagramTunnelImpl {
            stack,
            sequence: TempSeqGenerator::new(),
            vport,
            recv_buffer: Mutex::new(RecvBuffer {
                capability: recv_buffer,
                waker: None,
                buffer: LinkedList::new(),
            }),
            frag_buffer: Arc::new(Mutex::new(
                DatagramFragments::new(fragment_cache_size, fragment_expired_us)
            )),
        }));

        datagram_tunnel.fragment_timer(expired_tick_sec);

        return datagram_tunnel;
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

    fn package_max_len(&self, remote: &DeviceId) -> usize {
        let stack = Stack::from(&self.as_ref().stack);
        let tunnel = stack.tunnel_manager().container_of(remote);
        if let Some(tunnel) = tunnel {
            if tunnel.state() != TunnelState::Dead {
                return tunnel.mtu();
            }
        }

        return MTU-12;
    }

    fn send_datagram(
        &self,
        datagram: protocol::v0::Datagram,
        remote: &DeviceId, 
        plaintext: bool
    ) -> Result<(), std::io::Error> {
        let stack = Stack::from(&self.as_ref().stack);
        let tunnel = stack.tunnel_manager().container_of(remote);
        if let Some(tunnel) = tunnel {
            if tunnel.state() == TunnelState::Dead
                || tunnel.state() == TunnelState::Connecting {
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
                            remote_sn: None,
                            remote_desc: Some(remote_device),
                        };
                        let _ = tunnel.build_send(DynamicPackage::from(datagram), build_params, plaintext);
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
                tunnel.send_package(DynamicPackage::from(datagram), plaintext)
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
                        remote_sn: None,
                        remote_desc: Some(remote_device),
                    };
                    let _ = tunnel.build_send(DynamicPackage::from(datagram), build_params, plaintext);
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

    fn build_datagram(
        &self,
        buf: &[u8],
        options: &mut DatagramOptions,
        remote: &DeviceId,
        vport: u16,
        piece: Option<(u8, u8)>,
    ) -> protocol::v0::Datagram {
        let datagram = protocol::v0::Datagram {
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
            piece,
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
            inner_type: protocol::v0::DatagramType::Data,
            data: TailedOwnedData::from(buf),
        };

        trace!(
            "{} try send {} to {}:{}",
            self.as_ref(),
            datagram,
            remote,
            vport
        );

        datagram
    }

    pub fn send_to(
        &self,
        buf: &[u8],
        options: &mut DatagramOptions,
        remote: &DeviceId,
        vport: u16,
    ) -> Result<(), std::io::Error> {
        let mtu = MTU;
        let mut datagram = self.build_datagram(buf, options, remote, vport, None);
        let mut fragment_len = datagram.fragment_len(mtu, options.plaintext);

        if fragment_len == 0 {
            self.send_datagram(datagram, remote, options.plaintext)
        } else {
            if options.sequence.is_none() {
                let seq = self.0.sequence.generate();
                options.sequence = Some(seq);
                datagram.sequence = Some(seq);
                fragment_len = datagram.fragment_len(mtu, options.plaintext);
            }

            let count = (buf.len() as f64 / fragment_len as f64).ceil() as u8;
            let mut start = 0;
            let mut end = fragment_len;
            for i in 0..count {
                let datagram = self.build_datagram(&buf[start..end], options, remote, vport, Some((i, count)));
                let _ = self.send_datagram(datagram, remote, options.plaintext);

                start += fragment_len;
                end += fragment_len;
                if end > buf.len() {
                    end = buf.len();
                }
            }

            Ok(())
        }
    }

    pub fn vport(&self) -> u16 {
        self.0.vport
    }

    pub fn close(&self) {
        let stack = Stack::from(&self.0.stack);
        stack.datagram_manager().unbind(self.vport());
    }


    fn fragment_timer(&self, tick_sec: u64) {
        let frag_buffer = self.0.frag_buffer.clone();
        task::spawn(async move {
            loop {
                let fragments = frag_buffer.clone();
                task::sleep(Duration::from_secs(tick_sec)).await;
                {
                    let mut fragments = fragments.lock().unwrap();
                    fragments.expired_clear();
                }
            }
        });
    }

    fn on_datagram(
        &self,
        pkg: &protocol::v0::Datagram,
        from: &TunnelContainer, 
        plaintext: bool
    ) -> Result<OnPackageResult, BuckyError> {
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
                plaintext,
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

// FIXME: 整个 OnPackage体系的 package参数改成转移不是引用，这里就可以不用拷贝data
impl OnPackage<protocol::v0::Datagram, (&TunnelContainer, bool)> for DatagramTunnel {
    fn on_package(
        &self,
        pkg: &protocol::v0::Datagram,
        context: (&TunnelContainer, bool),
    ) -> Result<OnPackageResult, BuckyError> {
        let (from, plaintext) = context;
        log::trace!("{} recv {} from {}", self.as_ref(), pkg, from);
        assert_eq!(pkg.to_vport, self.vport());

        if pkg.piece.is_some()  {
            let reassemble_result = {
                let mut frag_buffer = self.0.frag_buffer.lock().unwrap();
                frag_buffer.reassemble(pkg, from)
            };
            match reassemble_result {
                Ok(ret) => {
                    if let Some(p) = ret {
                        self.on_datagram(&p, from, plaintext)
                    } else {
                        return Ok(OnPackageResult::Handled);
                    }
                }
                Err(_) => {
                    return Ok(OnPackageResult::Handled);
                }
            }
        } else {
            self.on_datagram(pkg, from, plaintext)
        }
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

impl DatagramFragments {
    pub fn new(frag_data_max_size: usize, frag_expired_us: u64) -> Self {
        DatagramFragments {
            fragments: HashMap::new(),
            frag_data_size: 0,
            frag_data_max_size,
            frag_expired_us
        }
    }

    pub fn expired_clear(&mut self) {
        let now = bucky_time_now();

        let mut clear_size = 0;
        for (_, fragment) in self.fragments.iter() {
            if fragment.expire_time < now {
                for (_, pkg) in fragment.datagrams.iter() {
                    clear_size += pkg.data.as_ref().len();
                }
            }
        }

        self.fragments.retain(|_, pkg| pkg.expire_time >= now);
        if clear_size > 0 {
            if self.frag_data_size < clear_size {
                error!("size wrong. frag_data_size={} clear_size={}", self.frag_data_size, clear_size);

                self.frag_data_size = 0;
            } else {
                info!("expired clear frag_data_size={} clear_size={}", self.frag_data_size, clear_size);

                self.frag_data_size -= clear_size;
            }
        }
    }

    pub fn reassemble(&mut self, pkg: &protocol::v0::Datagram, from: &TunnelContainer) -> BuckyResult<Option<protocol::v0::Datagram>> {
        if pkg.piece.is_none() || pkg.sequence.is_none() {
            return Ok(None);
        }

        let mut fragment_add_check = |pkg: &protocol::v0::Datagram, from: &TunnelContainer| -> bool {//check size
            let payload_size = pkg.data.as_ref().len();
            if self.frag_data_size + payload_size > self.frag_data_max_size {
                error!("fragment from={} from_vport={} to_vport={} sequence={:?} frage_data_size={} too many fragment, drop", 
                    from.remote(), pkg.from_vport, 
                    pkg.to_vport, 
                    pkg.sequence,
                    self.frag_data_size);
    
                return false;
            }

            self.frag_data_size += payload_size;

            return true;
        };

        let datagram_key = |pkg: &protocol::v0::Datagram, from: &TunnelContainer| -> String {
            format!("{}:{}:{}", from.remote(), pkg.from_vport, pkg.sequence.unwrap().value())
        };

        let payload_merge = |fragment: &DatagramFragment| -> protocol::v0::Datagram { 
            let mut payload_size = 0;
            for i in 0..fragment.fragment_total {
                let n = i as u8;
                let frag = fragment.datagrams.get(&n).unwrap();
                payload_size += frag.data.as_ref().len();
            }
    
            let mut payload = vec![0u8;payload_size];
            let mut pos = 0;
            for i in 0..fragment.fragment_total {
                let n = i as u8;
                let frag = fragment.datagrams.get(&n).unwrap();
                let len = frag.data.as_ref().len();
                payload[pos..pos+len].copy_from_slice(frag.data.as_ref());
                pos += len;
            }

            let pkg = fragment.datagrams.get(&0).unwrap();
            protocol::v0::Datagram {
                to_vport: pkg.to_vport,
                from_vport: pkg.from_vport,
                dest_zone: pkg.dest_zone.clone(),
                hop_limit: pkg.hop_limit.clone(),
                sequence: pkg.sequence.clone(),
                piece: pkg.piece.clone(),
                send_time: pkg.send_time.clone(),
                create_time: pkg.create_time.clone(),
                author_id: pkg.author_id.clone(),
                author: pkg.author.clone(),
                inner_type: pkg.inner_type,
                data: TailedOwnedData::from(payload),
            }
        };

        let key = datagram_key(pkg, from);
        if let Some(fragment) = self.fragments.get_mut(&key) {
            let (fragment_index, _) = pkg.piece.unwrap();
            if let Some(_) = fragment.datagrams.get(&fragment_index) {//duplicate
                return Ok(None);
            }

            if !fragment_add_check(pkg, from) {
                return Ok(None);
            }

            fragment.datagrams.insert(fragment_index, pkg.clone());

            if fragment.datagrams.len() == fragment.fragment_total {//complete
                let pkg = payload_merge(fragment);
                self.fragments.remove(&key);
                if self.frag_data_size < pkg.data.as_ref().len() {
                    error!("size wrong. frag_data_size={} pkg_data={}", self.frag_data_size, pkg.data.as_ref().len());

                    self.frag_data_size = 0;
                } else {
                    self.frag_data_size -= pkg.data.as_ref().len();
                }

                return Ok(Some(pkg))
            }

            return Ok(None);
        }

        //new
        if !fragment_add_check(pkg, from) {
            return Ok(None);
        }

        let expire_time = bucky_time_now() + self.frag_expired_us;
        let (fragment_index, fragment_total) = pkg.piece.unwrap();

        let mut fragment = DatagramFragment {
            author_id: from.remote().clone(),
            from_vport: pkg.from_vport,
            sequence: pkg.sequence.unwrap(),
            to_vport: pkg.to_vport,
            expire_time,
            datagrams: HashMap::new(),
            fragment_total: fragment_total as usize,
        };

        fragment.datagrams.insert(fragment_index, pkg.clone());

        self.fragments.insert(datagram_key(pkg, from), fragment);

        Ok(None)
    }
}
