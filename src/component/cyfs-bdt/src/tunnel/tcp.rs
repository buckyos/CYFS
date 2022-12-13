use log::*;
use std::{
    sync::atomic::{AtomicI32, AtomicU64, Ordering},  
    time::Duration
};
use cyfs_debug::Mutex;
use async_std::{
    sync::{Arc}, 
    channel::{bounded, Sender, Receiver}, 
    task, 
    future
};
use futures::future::{Abortable, AbortHandle, AbortRegistration};
use async_trait::{async_trait};
use ringbuf;
use cyfs_base::*;
use crate::{
    types::*,
    protocol::{self, *, v0::*},
    history::keystore, 
    MTU,
    interface::{self, *, tcp::{OnTcpInterface, RecvBox, PackageInterface}}
};
use super::{tunnel::{self, DynamicTunnel, TunnelOwner, ProxyType}, TunnelContainer};

#[derive(Clone)]
pub struct Config {
    pub connect_timeout: Duration, 
    pub confirm_timeout: Duration, 
    pub accept_timeout: Duration,
    // 调用retain_keeper 之后延迟多久开始尝试从preactive 进入 active状态
    pub retain_connect_delay: Duration, 
    pub ping_interval: Duration, 
    pub ping_timeout: Duration,

    pub package_buffer: usize, 
    pub piece_buffer: usize,
    // 检查发送piece buffer的间隔
    pub piece_interval: Duration,
}

enum TunnelState {
    Connecting(ConnectingState),
    // 通过历史判定可以联通，但是并没有创建 Interface的状态
    // 当通过Tunnel发包时，进入Connecting状态去连接 
    PreActive(PreActiveState), 
    Active(ActiveState), 
    Dead, 
} 


// #[derive(Clone, Copy)]
enum ConnectorState {
    None, 
    Connecting, 
    ReverseConnecting(AbortHandle)
}

struct ConnectingState {
    owner: TunnelContainer, 
    connector: ConnectorState 
}



enum PackageElem {
    Package(DynamicPackage), 
    RawData(Vec<u8>),  
}

enum CommandElem {
    Discard(usize)
}

enum SignalElem {
    Package(PackageElem), 
    Command(CommandElem)
}


struct PreActiveState {
    owner: TunnelContainer, 
    connector: ConnectorState, 
    remote_timestamp: Timestamp, 
    signal_writer: Sender<SignalElem>,
    signal_reader: Receiver<SignalElem>, 
}

struct ActiveState {
    owner: TunnelContainer, 
    interface: tcp::PackageInterface, 
    remote_timestamp: Timestamp, 
    syn_seq: TempSeq, 
    signal_writer: Sender<SignalElem>,
    piece_writer: ringbuf::Producer<u8>, 
    dead_waiters: StateWaiter
}

struct TunnelImpl {
    remote_device_id: DeviceId, 
    local_remote: EndpointPair, 
    keeper_count: AtomicI32, 
    last_active: AtomicU64, 
    retain_connect_timestamp: AtomicU64, 
    state: Mutex<TunnelState>,
    mtu: usize,
}

#[derive(Clone)]
pub struct Tunnel(Arc<TunnelImpl>);

impl std::fmt::Display for Tunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TcpTunnel{{remote_device:{}, local:{}, remote:{}}}", self.0.remote_device_id, tunnel::Tunnel::local(self), tunnel::Tunnel::remote(self))
    }
}

impl Tunnel {
    pub fn new(
        owner: TunnelContainer, 
        ep_pair: EndpointPair) -> Self {
        let remote_device_id = owner.remote().clone();
        let tunnel = Self(Arc::new(TunnelImpl {
            mtu: MTU-12, 
            remote_device_id, 
            local_remote: ep_pair, 
            keeper_count: AtomicI32::new(0), 
            last_active: AtomicU64::new(0), 
            retain_connect_timestamp: AtomicU64::new(0), 
            state: Mutex::new(TunnelState::Connecting(
                ConnectingState {
                    owner, 
                    connector: ConnectorState::None
                }))
        }));
        info!("{} created with state: {:?}", tunnel, tunnel::Tunnel::state(&tunnel));
        tunnel
    }

    pub fn pre_active(&self, remote_timestamp: Timestamp) -> BuckyResult<TunnelContainer> {
        self.0.last_active.store(bucky_time_now(), Ordering::SeqCst);
        struct NextStep {
            owner: TunnelContainer, 
            former_state: tunnel::TunnelState, 
            cur_state: tunnel::TunnelState
        }
        let next_step = {
            let state = &mut *self.0.state.lock().unwrap();
            match state {
                TunnelState::Connecting(connecting) => {
                    info!("{} Connecting=>PreActive", self);
                    let owner = connecting.owner.clone();
                    let (signal_writer, signal_reader) = bounded(owner.config().tcp.package_buffer);
                    *state = TunnelState::PreActive(PreActiveState {
                        owner: owner.clone(), 
                        remote_timestamp, 
                        connector: match connecting.connector {
                            ConnectorState::None => ConnectorState::None, 
                            ConnectorState::Connecting => ConnectorState::Connecting, 
                            ConnectorState::ReverseConnecting(_) => unreachable!()
                        },
                        signal_writer, 
                        signal_reader
                    });
                    Ok(NextStep {
                        owner, 
                        former_state: tunnel::TunnelState::Connecting, 
                        cur_state: tunnel::TunnelState::Active(remote_timestamp)})
                }, 
                TunnelState::PreActive(pre_active) => {
                    if pre_active.remote_timestamp > remote_timestamp {
                        Ok((tunnel::TunnelState::Active(remote_timestamp), tunnel::TunnelState::Active(remote_timestamp)))
                    } else if pre_active.remote_timestamp == remote_timestamp {
                        Ok((tunnel::TunnelState::Active(remote_timestamp), tunnel::TunnelState::Active(remote_timestamp)))
                    } else {
                        let former_state = tunnel::TunnelState::Active(pre_active.remote_timestamp);
                        pre_active.remote_timestamp = remote_timestamp;
                        Ok((former_state, tunnel::TunnelState::Active(remote_timestamp)))
                    }.map(|(former_state, cur_state)| NextStep {
                        owner: pre_active.owner.clone(), 
                        former_state, 
                        cur_state 
                    })
                }, 
                TunnelState::Active(active) => {
                    if active.remote_timestamp < remote_timestamp {
                        info!("{} Active=>PreActive", self);
                        let owner = active.owner.clone();
                        let (signal_writer, signal_reader) = bounded(owner.config().tcp.package_buffer);
                        let former_state = tunnel::TunnelState::Active(active.remote_timestamp);
                        *state = TunnelState::PreActive(PreActiveState {
                            owner: owner.clone(), 
                            remote_timestamp, 
                            connector: ConnectorState::None, 
                            signal_writer, 
                            signal_reader
                        });
                        Ok(NextStep {
                            owner, 
                            former_state, 
                            cur_state: tunnel::TunnelState::Active(remote_timestamp)
                        })
                    } else {
                        Ok(NextStep {
                            owner: active.owner.clone(), 
                            former_state: tunnel::TunnelState::Active(active.remote_timestamp), 
                            cur_state: tunnel::TunnelState::Active(active.remote_timestamp)
                        })
                    }
                },
                TunnelState::Dead => Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel dead"))
            }
        }?;
        if next_step.former_state != next_step.cur_state {
            next_step.owner.sync_tunnel_state(
                &DynamicTunnel::new(self.clone()), 
                next_step.former_state, 
                next_step.cur_state);
        }
        Ok(next_step.owner)
    }

    pub fn is_reverse(&self) -> bool {
        tunnel::Tunnel::remote(self).addr().port() == 0
    }

    pub fn is_data_piece_full(&self) -> BuckyResult<bool> {
        let state = &mut *self.0.state.lock().unwrap();
        match state {
            TunnelState::Active(active) => {
                Ok(active.piece_writer.capacity() == active.piece_writer.len()) 
            }, 
            _ => Err(BuckyError::new(BuckyErrorCode::ErrorState, "not active"))
        }
    }

    pub fn discard_data_piece(&self) -> BuckyResult<()> {
        let (signal_writer, len) = {
            let state = &mut *self.0.state.lock().unwrap();
            match state {
                TunnelState::Active(active) => Ok((active.signal_writer.clone(), active.piece_writer.len())), 
                _ => Err(BuckyError::new(BuckyErrorCode::ErrorState, "not active"))
            }
        }?;
        if len > 0 {
            info!("{} send discard command: {}", self, len);
            let _ = signal_writer.try_send(SignalElem::Command(CommandElem::Discard(len)));
        }
        Ok(())
    }

    pub fn send_data_piece(&self, buf: &[u8]) -> BuckyResult<()> {
        let state = &mut *self.0.state.lock().unwrap();
        match state {
            TunnelState::Active(active) => {
                if active.piece_writer.remaining() >= buf.len() {
                    let len = active.piece_writer.push_slice(buf);
                    assert_eq!(len, buf.len());
                    Ok(())
                } else {
                    Err(BuckyError::new(BuckyErrorCode::Pending, "full"))
                }
            }, 
            _ => Err(BuckyError::new(BuckyErrorCode::ErrorState, "not active"))
        }
    }

    fn active_with_interface(&self, interface: Result<(tcp::PackageInterface, Timestamp, TempSeq), BuckyError>) {
        match interface {
            Ok((interface, remote_timestamp, syn_seq)) => {
                struct NextStep {
                    owner: TunnelContainer, 
                    former_state: tunnel::TunnelState, 
                    cur_state: tunnel::TunnelState, 
                    signal_reader: Receiver<SignalElem>, 
                    piece_reader: ringbuf::Consumer<u8>, 
                    reverse_waiter: Option<AbortHandle>, 
                    to_close: Option<tcp::PackageInterface>, 
                    dead_waiter: AbortRegistration
                }
                self.0.last_active.store(bucky_time_now(), Ordering::SeqCst);
                if let Some(next_step) = {
                    let state = &mut *self.0.state.lock().unwrap();
                    match state {
                        TunnelState::Connecting(connecting) => {
                            info!("{} connecting => active(remote:{}, seq:{:?})", self, remote_timestamp, syn_seq);
                            let owner = connecting.owner.clone();
                            let (signal_writer, signal_reader) = bounded(owner.config().tcp.package_buffer);
                            let ring_buf = ringbuf::RingBuffer::<u8>::new(owner.config().tcp.piece_buffer * udp::MTU);
                            let (piece_writer, piece_reader) = ring_buf.split();
                            let mut dead_waiters = StateWaiter::new();
                            let dead_waiter = dead_waiters.new_waiter();
                            *state = TunnelState::Active(ActiveState {
                                owner: owner.clone(), 
                                interface: interface.clone(), 
                                remote_timestamp, 
                                syn_seq, 
                                signal_writer, 
                                piece_writer, 
                                dead_waiters
                            });
                            
                            Some(NextStep {
                                owner, 
                                former_state: tunnel::TunnelState::Connecting, 
                                cur_state: tunnel::TunnelState::Active(remote_timestamp), 
                                signal_reader, 
                                piece_reader,   
                                reverse_waiter: None, 
                                to_close: None, 
                                dead_waiter
                            })
                        }, 
                        TunnelState::PreActive(pre_active) => {
                            //FIXME: 检查 preactive 的 remote timestamp 和  active 的 remote timestamp
                            info!("{} PreActive => Active(remote:{}, seq:{:?})", self, remote_timestamp, syn_seq);
                            let former_state = tunnel::TunnelState::Active(pre_active.remote_timestamp);
                            let owner = pre_active.owner.clone();

                            let ring_buf = ringbuf::RingBuffer::<u8>::new(owner.config().tcp.piece_buffer * udp::MTU);
                            let (piece_writer, piece_reader) = ring_buf.split();

                            let signal_reader = pre_active.signal_reader.clone();
                            let signal_writer = pre_active.signal_writer.clone();

                            let mut dead_waiters = StateWaiter::new();
                            let dead_waiter = dead_waiters.new_waiter();

                            let reverse_waiter = match &pre_active.connector {
                                ConnectorState::ReverseConnecting(waiter) => {
                                    Some(waiter.clone())
                                }, 
                                _ => None
                            };
                            *state = TunnelState::Active(ActiveState {
                                owner: owner.clone(), 
                                interface: interface.clone(),  
                                remote_timestamp, 
                                syn_seq, 
                                signal_writer, 
                                piece_writer, 
                                dead_waiters
                            });
                            Some(NextStep {
                                owner, 
                                former_state, 
                                cur_state: tunnel::TunnelState::Active(remote_timestamp), 
                                signal_reader, 
                                piece_reader, 
                                reverse_waiter, 
                                to_close: None, 
                                dead_waiter
                            })
                        },
                        TunnelState::Active(active) => {
                            if active.remote_timestamp < remote_timestamp 
                                || active.syn_seq < syn_seq {
                                info!("{} Active(remote:{}, seq:{:?}) => Active(remote:{}, seq:{:?})", self, active.remote_timestamp, active.syn_seq, remote_timestamp, syn_seq);
                                let former_state = tunnel::TunnelState::Active(active.remote_timestamp);
                                let owner = active.owner.clone();
    
                                let ring_buf = ringbuf::RingBuffer::<u8>::new(owner.config().tcp.piece_buffer * udp::MTU);
                                let (piece_writer, piece_reader) = ring_buf.split();
                                let (signal_writer, signal_reader) = bounded(owner.config().tcp.package_buffer);
                                let to_close = Some(active.interface.clone());
                                
                                let mut dead_waiters = StateWaiter::new();
                                let dead_waiter = dead_waiters.new_waiter();

                                *state = TunnelState::Active(ActiveState {
                                    owner: owner.clone(), 
                                    interface: interface.clone(),  
                                    remote_timestamp, 
                                    syn_seq, 
                                    signal_writer, 
                                    piece_writer, 
                                    dead_waiters
                                });
                                Some(NextStep {
                                    owner, 
                                    former_state, 
                                    cur_state: tunnel::TunnelState::Active(remote_timestamp), 
                                    signal_reader, 
                                    piece_reader, 
                                    reverse_waiter: None, 
                                    to_close, 
                                    dead_waiter
                                })
                            } else {
                                None
                            }
                        },
                        _ => None
                    }
                } {
                    if let Some(reverse_waiter) = next_step.reverse_waiter {
                        reverse_waiter.abort();
                    }
                    self.start_recv(next_step.owner.clone(), interface, next_step.dead_waiter);
                    self.start_send(next_step.owner.clone(), next_step.signal_reader, next_step.piece_reader);

                    if next_step.former_state != next_step.cur_state {
                        next_step.owner.sync_tunnel_state(&DynamicTunnel::new(self.clone()), next_step.former_state, next_step.cur_state);
                    }

                    if let Some(to_close) = next_step.to_close {
                        info!("{} will close older {}", self, to_close);
                        to_close.close();
                    }
                }
            }, 
            Err(err) => {
                info!("{} dead for {}", self, err);
                if let Some((owner, former_state)) = {
                    let state = &mut *self.0.state.lock().unwrap();
                    match state {
                        TunnelState::Connecting(connecting) => {
                            info!("{} connecting => dead", self);
                            let owner = connecting.owner.clone();
                            *state = TunnelState::Dead;
                            Some((owner, tunnel::TunnelState::Connecting))
                        }, 
                        TunnelState::PreActive(pre_active) => {
                            info!("{} PreActive => dead", self);
                            let former_state = tunnel::TunnelState::Active(pre_active.remote_timestamp);
                            let owner = pre_active.owner.clone();
                            *state = TunnelState::Dead;
                            Some((owner, former_state))
                        },
                        TunnelState::Active(_) => {
                            None
                        },
                        _ => {
                            // do nothing
                            None
                        }
                    }
                } {
                    owner.sync_tunnel_state(&DynamicTunnel::new(self.clone()), former_state, tunnel::TunnelState::Dead);
                }
            }
        }
    }

    pub(super) fn connect(&self) -> Result<(), BuckyError> {
        if !self.is_reverse() {
            info!("{} connect", self);
            let owner = {
                let state = &mut *self.0.state.lock().unwrap();
                match state {
                    // build tunnel的时候会从Connecting状态调用Connect
                    TunnelState::Connecting(connecting) => {
                        match connecting.connector {
                            ConnectorState::None => {
                                connecting.connector = ConnectorState::Connecting;
                                Ok(connecting.owner.clone())
                            }, 
                            _ => {
                                Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "connector exists"))
                            }
                        } 
                    }, 
                    TunnelState::PreActive(pre_active) => {
                        match pre_active.connector {
                            ConnectorState::None => {
                                pre_active.connector = ConnectorState::Connecting;
                                Ok(pre_active.owner.clone())
                            }, 
                            _ => {
                                Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "connector exists"))
                            }
                        } 
                    },
                    TunnelState::Active(_) => {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel already active"))
                    }, 
                    TunnelState::Dead => {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel already dead"))
                    }
                }
            }.map_err(|err| {debug!("{} connect failed for {}", self, err); err})?;
            
            let tunnel = self.clone();
            task::spawn(async move {
                tunnel.active_with_interface(tunnel.connect_inner(owner.clone(), None).await);
            });
                
        } else {
            info!("{} reverse connect", self);
            let (owner, reg) = {
                let state = &mut *self.0.state.lock().unwrap();
                match state {
                    TunnelState::Connecting(_) => {
                        unreachable!()
                    }, 
                    TunnelState::PreActive(pre_active) => {
                        match pre_active.connector {
                            ConnectorState::None => {
                                let (waiter, reg) = AbortHandle::new_pair();
                                pre_active.connector = ConnectorState::ReverseConnecting(waiter);
                                Ok((pre_active.owner.clone(), reg))
                            }, 
                            _ => {
                                debug!("{} ignore reverse connect for reverse connecting", self);
                                Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "connector exists"))
                            }
                        } 
                    },
                    TunnelState::Active(_) => {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel already active"))
                    }, 
                    TunnelState::Dead => {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel already dead"))
                    }
                }
            }.map_err(|err| {debug!("{} connect failed for {}", self, err); err})?;

            let tunnel = self.clone();
            task::spawn(async move {
                match tunnel.reverse_connect_inner(owner, reg).await {
                    Ok(_) => {
                        // do nothing
                    }, 
                    Err(err) => {
                        info!("{} reverse connect failed for {}", tunnel, err);
                        tunnel.active_with_interface(Err(err));
                    }
                };
            });
        }
        Ok(())
    }

    pub(crate) fn connect_with_interface(&self, interface: tcp::Interface) -> Result<(), BuckyError> {
        info!("{} connect_with_interface", self);
        let owner = {
            let state = &mut *self.0.state.lock().unwrap();
            match state {
                TunnelState::Connecting(connecting) => {
                    match connecting.connector {
                        ConnectorState::None => {
                            connecting.connector = ConnectorState::Connecting;
                            Ok(connecting.owner.clone())
                        }, 
                        _ => {
                            Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "connector exists"))
                        }
                    } 
                }, 
                TunnelState::PreActive(_) => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel already active"))
                },
                TunnelState::Active(_) => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel already active"))
                }, 
                TunnelState::Dead => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel already dead"))
                }
            }
        }.map_err(|err| {debug!("{} connect failed for {}", self, err); err})?;
        
        let tunnel = self.clone();
        task::spawn(async move {
            
            tunnel.active_with_interface(tunnel.connect_inner(owner.clone(), Some(interface)).await);
        });
        Ok(())
    }

    async fn connect_inner(&self, owner: TunnelContainer, interface: Option<tcp::Interface>) -> Result<(tcp::PackageInterface, Timestamp, TempSeq), BuckyError> {
        info!("{} connect interface", self);
        let stack = owner.stack();
        let key_stub = stack.keystore().create_key(owner.remote_const(), true);
        let interface = if let Some(interface) = interface {
            Ok(interface)
        } else {
            tcp::Interface::connect(
            // tunnel::Tunnel::local(self).addr().ip(), 
            *tunnel::Tunnel::remote(self), 
            owner.remote().clone(), 
            owner.remote_const().clone(), 
            key_stub.key, 
            owner.config().tcp.connect_timeout).await
        }?;
        let syn_seq = owner.generate_sequence();
        let syn_tunnel = SynTunnel {
            protocol_version: owner.protocol_version(), 
            stack_version: owner.stack_version(),  
            to_device_id: owner.remote().clone(),
            sequence: syn_seq.clone(),
            from_device_desc: stack.local().clone(),
            send_time: bucky_time_now()
        };
        let resp_box = interface.confirm_connect(&stack, vec![DynamicPackage::from(syn_tunnel)], owner.config().tcp.confirm_timeout).await?;
        
        if resp_box.packages().len() != 1 {
            Err(BuckyError::new(BuckyErrorCode::InvalidData, "should response AckTunnel"))
        } else if resp_box.packages()[0].cmd_code() != PackageCmdCode::AckTunnel {
            Err(BuckyError::new(BuckyErrorCode::InvalidData, "should response AckTunnel"))
        } else {
            let ack_tunnel: &AckTunnel = resp_box.packages()[0].as_ref();
            let _ = owner.on_package(ack_tunnel, None);
            if ack_tunnel.result == ACK_TUNNEL_RESULT_OK {
                let remote_timestamp = ack_tunnel.to_device_desc.body().as_ref().unwrap().update_time();
                Ok((interface.into(), remote_timestamp, syn_seq))
            } else if ack_tunnel.result == ACK_TUNNEL_RESULT_REFUSED {
                Err(BuckyError::new(BuckyErrorCode::InvalidData, "refused"))
            } else {
                Err(BuckyError::new(BuckyErrorCode::InvalidData, "should response AckTunnel"))
            }            
        }
    }

    async fn reverse_connect_inner(&self, owner: TunnelContainer, reg: AbortRegistration) -> Result<(), BuckyError> {
        let stack = owner.stack();
        let remote = stack.device_cache().get(owner.remote()).await.ok_or_else(| | BuckyError::new(BuckyErrorCode::NotFound, "device not cached"))?;
        let sn_id = remote.connect_info().sn_list().get(0).ok_or_else(| | BuckyError::new(BuckyErrorCode::NotFound, "device no sn"))?;
        let sn = stack.device_cache().get(sn_id).await.ok_or_else(| | BuckyError::new(BuckyErrorCode::NotFound, "sn not cached"))?;

        let key_stub = stack.keystore().create_key(owner.remote_const(), true);
        let mut syn_box = PackageBox::encrypt_box(owner.remote().clone(), key_stub.key.clone());
        let syn_tunnel = SynTunnel {
            protocol_version: owner.protocol_version(), 
            stack_version: owner.stack_version(),  
            to_device_id: owner.remote().clone(),
            sequence: owner.generate_sequence(),
            from_device_desc: stack.local().clone(),
            send_time: bucky_time_now()
        };
        if let keystore::EncryptedKey::Unconfirmed(encrypted) = key_stub.encrypted {
            let mut exchg = Exchange::from((&syn_tunnel, encrypted, key_stub.key.mix_key));
            exchg.sign(stack.keystore().signer()).await?;
            syn_box.push(exchg);
        }
        syn_box.push(syn_tunnel);

        let listener = stack.net_manager().listener();
        let mut endpoints = vec![];
        for t in listener.tcp() {
            let outer = t.outer();
            if outer.is_some() {
                let outer = outer.unwrap();
                if outer.eq(tunnel::Tunnel::local(self)) 
                    || t.local().eq(tunnel::Tunnel::local(self)) {
                    endpoints.push(outer);
                } 
            } else {
                endpoints.push(*tunnel::Tunnel::local(self));
            }
        }

        let _ = stack.sn_client().call(
            &endpoints, 
            owner.remote(), 
            &sn, 
            true, 
            true,
            true,
            |sn_call| {
                let mut context = udp::PackageBoxEncodeContext::from(sn_call);
                let mut buf = vec![0u8; interface::udp::MTU_LARGE];
                let enc_len = syn_box.raw_tail_encode_with_context(&mut buf, &mut context, &None).unwrap().len();
                buf.truncate(enc_len);
                buf
            }).await;
                
        let waiter = Abortable::new(future::pending::<()>(), reg);
        let _ = future::timeout(owner.config().connect_timeout, waiter).await?;
        Ok(())
    }

    fn on_interface_error(&self, from: &PackageInterface, err: &BuckyError) {
        error!("{} interface error {} from {}", self, err, from);

        let notify = {
            let state = &mut *self.0.state.lock().unwrap();
            match state { 
                TunnelState::Active(active) => {
                    let owner = active.owner.clone();
                    if active.interface.ptr_eq(from) {
                        info!("{} Active({})=>Dead for interface error", self, active.remote_timestamp);
                        let former_state = tunnel::TunnelState::Active(active.remote_timestamp);
                        let mut dead_waiters = StateWaiter::new();
                        std::mem::swap(&mut dead_waiters, &mut active.dead_waiters);
                        *state = TunnelState::Dead;
                        Some((owner, former_state, Some(dead_waiters)))
                    } else {
                        None
                    }
                }, 
                _ => None
            }
        };

        if let Some((owner, former_state, dead_waiters)) = notify {
            if let Some(dead_waiters) = dead_waiters {
                dead_waiters.wake();
            }
            owner.sync_tunnel_state(&DynamicTunnel::new(self.clone()), former_state, tunnel::TunnelState::Dead);
        }
        
    }

    fn start_send(
        &self, 
        owner: TunnelContainer, 
        signal_reader: Receiver<SignalElem>, 
        piece_reader: ringbuf::Consumer<u8>,  
        ) {
        let tunnel = self.clone();
        task::spawn(async move {
            let stub = {
                match &*tunnel.0.state.lock().unwrap() {
                    TunnelState::Active(active) => {
                        Ok(active.interface.clone())
                    }, 
                    _ => {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, "break send loop for invalid state"))
                    }
                }
            };
            
            if stub.is_err() {
                return ;
            }
            let interface = stub.unwrap();
            let mut piece_reader = piece_reader;

            info!("{} send loop start, {}", tunnel, owner.config().tcp.piece_interval.as_millis());
            loop {
                let mut send_buf = [0u8; udp::MTU_LARGE];
                let mut piece_buf = [0u8; udp::MTU];
              
                fn handle_command(
                    piece_reader: &mut ringbuf::Consumer<u8>, 
                    command: CommandElem 
                ) -> BuckyResult<()> {
                    match command {
                        CommandElem::Discard(len) => piece_reader.discard(len),
                    };
                    Ok(())
                }

                async fn handle_package(
                    interface: &PackageInterface, 
                    send_buf: &mut [u8], 
                    pkg: PackageElem) -> BuckyResult<()> {
                    match pkg {
                        PackageElem::Package(package) => interface.send_package(send_buf, package, false).await, 
                        PackageElem::RawData(data) => interface.send_raw_data(data).await
                    }
                }

                async fn handle_signal(
                    interface: &PackageInterface, 
                    piece_reader: &mut ringbuf::Consumer<u8>, 
                    send_buf: &mut [u8], 
                    signal: SignalElem
                ) -> BuckyResult<()> {
                    match signal {
                        SignalElem::Package(package) => handle_package(interface, send_buf, package).await, 
                        SignalElem::Command(command) => handle_command(piece_reader, command)
                    }
                }


                async fn handle_piece(
                    interface: &PackageInterface, 
                    send_buf: &mut [u8], 
                    piece_reader: &mut ringbuf::Consumer<u8>, 
                    signal_reader: &Receiver<SignalElem>) -> BuckyResult<()> {
                    loop {
                        let len = piece_reader.pop_slice(send_buf);
                        if len > 0 {
                            assert_eq!(len, send_buf.len());
                            let (box_len, _) = u16::raw_decode(send_buf).unwrap();
                            match interface.send_raw_buffer(&mut send_buf[..u16::raw_bytes().unwrap() + box_len as usize]).await {
                                Ok(_) => {
                                    //continue
                                }, 
                                Err(err) => {
                                    break Err(err);
                                }
                            }
                        } else {
                            break Ok(());
                        }
                        if signal_reader.len() > 0 {
                            // 优先处理package
                            break Ok(());
                        }
                    }
                }

                match future::timeout(owner.config().tcp.piece_interval, signal_reader.recv()).await {
                    Ok(recv) => {
                        match recv {
                            Ok(signal) => {
                                match handle_signal(&interface, &mut piece_reader, &mut send_buf, signal).await {
                                    Ok(_) => {
                                        if signal_reader.len() == 0 {
                                            match handle_piece(&interface, &mut piece_buf, &mut piece_reader, &signal_reader).await {
                                                Ok(_) => {
                                                    // continue
                                                }, 
                                                Err(err) => {
                                                    tunnel.on_interface_error(&interface, &err);
                                                    info!("{} send loop break for err {}", tunnel, err);
                                                    break;
                                                }
                                            }
                                        }
                                    }, 
                                    Err(err) => {
                                        tunnel.on_interface_error(&interface, &err);
                                        info!("{} send loop break for err {}", tunnel, err);
                                        break;
                                    }
                                }
                            }, 
                            Err(err) => {
                                info!("{} send loop break for err {}", tunnel, err);
                                break;
                            }
                        }
                    }
                    Err(_err) => {
                        match handle_piece(&interface, &mut piece_buf, &mut piece_reader, &signal_reader).await {
                            Ok(_) => {
                                // continue
                            }, 
                            Err(err) => {
                                tunnel.on_interface_error(&interface, &err);
                                info!("{} send loop break for err {}", tunnel, err);
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    async fn recv_inner(
        tunnel: Self, 
        owner: TunnelContainer, 
        interface: tcp::PackageInterface) {
        // recv loop
        let mut recv_buf = [0u8; udp::MTU_LARGE];
        loop {
            // tunnel显式销毁时，需要shutdown tcp stream; 这里receive_package就会出错了
            match interface.receive_package(&mut recv_buf).await {
                Ok(recv_box) => {
                    // tunnel.0.last_active.store(bucky_time_now(), Ordering::SeqCst);

                    match recv_box {
                        RecvBox::Package(package_box) => {
                            let stack = owner.stack();
                            if package_box.has_exchange() {
                                // let exchange: &Exchange = package_box.packages()[0].as_ref();
                                stack.keystore().add_key(package_box.key(), package_box.remote());
                            }
                            if let Err(err) = package_box.packages().iter().try_for_each(|pkg| {
                                if pkg.cmd_code() == PackageCmdCode::PingTunnel {
                                    tunnel.on_package(AsRef::<PingTunnel>::as_ref(pkg), None).map(|_| ())
                                } else if pkg.cmd_code() == PackageCmdCode::PingTunnelResp {
                                    tunnel.on_package(AsRef::<PingTunnelResp>::as_ref(pkg), None).map(|_| ())
                                } else {
                                    downcast_session_handle!(pkg, |pkg| owner.on_package(pkg, None)).map(|_| ())
                                }
                            }) {
                                warn!("{} package error {}", tunnel, err);
                            }
                        }, 
                        RecvBox::RawData(raw_data) => {
                            let _ = owner.on_raw_data(raw_data, DynamicTunnel::new(tunnel.clone()));
                        }
                    }
                }, 
                Err(err) => {
                    tunnel.on_interface_error(&interface, &err);
                    break;
                }
            }
        }
    }

    fn start_recv(
        &self, 
        owner: TunnelContainer, 
        interface: tcp::PackageInterface, 
        dead_waiter: AbortRegistration) {
        let (cancel, reg) = AbortHandle::new_pair();
        task::spawn(Abortable::new(Self::recv_inner(self.clone(), owner, interface), reg)); 
        let tunnel = self.clone();
        task::spawn(async move {
            let _ = StateWaiter::wait(dead_waiter, || ()).await;
            error!("{} break recv loop for tunnel dead", tunnel);
            cancel.abort();
        });
    }

    async fn retain_connect(
        &self, 
        retain_connect_timestamp: Timestamp, 
        ping_interval: Duration, 
        ping_timeout: Duration) {
        if self.0.retain_connect_timestamp.load(Ordering::SeqCst) != retain_connect_timestamp {
            debug!("ignore retain connect for timestamp missmatch, tunnel:{}", self);
            return ;
        }
        if self.0.keeper_count.load(Ordering::SeqCst) == 0 {
            debug!("ignore retain connect for zero retain count, tunnel:{}", self);
            return ;
        }

        if !self.is_reverse() {
            info!("begin retain connect, tunnel:{}", self);
            let _ = self.connect();
        } 

        let tunnel = self.clone();
        task::spawn(async move {
            loop {
                if tunnel.0.keeper_count.load(Ordering::SeqCst) == 0 {
                    info!("break ping loop for release keeper, tunnel:{}", tunnel);
                    break;
                }
                match {
                    let state = &*tunnel.0.state.lock().unwrap();
                    match state {
                        TunnelState::Active(active) => {
                            Ok(Some(active.owner.clone()))
                        }, 
                        TunnelState::Dead => {
                            Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's dead"))
                        }, 
                        _ => {
                            Ok(None)
                        }
                    }
                } {
                    Ok(owner) => {
                        if owner.is_some() {
                            let now = bucky_time_now();
                            let miss_active_time = Duration::from_micros(now - tunnel.0.last_active.load(Ordering::SeqCst));
                            if miss_active_time > ping_timeout {
                                if let Some((owner, cur_state, dead_waiters)) = {
                                    let state = &mut *tunnel.0.state.lock().unwrap();
                                    if let TunnelState::Active(active) = state {
                                        error!("dead for ping timeout, tunnel:{}", tunnel);
                                        let cur_state = tunnel::TunnelState::Active(active.remote_timestamp);
                                        let owner = active.owner.clone();
                                        let mut dead_waiters = StateWaiter::new();
                                        std::mem::swap(&mut dead_waiters, &mut active.dead_waiters);
                                        *state = TunnelState::Dead;
                                        Some((owner, cur_state, dead_waiters))
                                    } else {
                                        None
                                    }
                                } {
                                    dead_waiters.wake();
                                    owner.sync_tunnel_state(&tunnel::DynamicTunnel::new(tunnel.clone()), cur_state, tunnel::Tunnel::state(&tunnel));
                                }
                                break;
                            }
                            if miss_active_time > ping_interval {
                                if tunnel.0.keeper_count.load(Ordering::SeqCst) > 0 {
                                    info!("send ping, tunnel:{}", tunnel);
                                    let ping = PingTunnel {
                                        package_id: 0,
                                        send_time: now,
                                        recv_data: 0,
                                    };
                                    let _ = tunnel::Tunnel::send_package(&tunnel, DynamicPackage::from(ping));
                                }
                            }
                        }
                        let _ = future::timeout(ping_interval, future::pending::<()>()).await;
                    }, 
                    Err(e) => {
                        error!("break ping loop, tunnel:{}, err:{}", tunnel, e);
                        break;
                    }
                }
            };
        });
    }
}

#[async_trait]
impl tunnel::Tunnel for Tunnel {
    fn mtu(&self) -> usize {
        self.0.mtu
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn local(&self) -> &Endpoint {
        self.0.local_remote.local()
    }

    fn remote(&self) -> &Endpoint {
        self.0.local_remote.remote()
    }

    fn proxy(&self) -> ProxyType {
        ProxyType::None
    }

    fn state(&self) -> tunnel::TunnelState {
        match &*self.0.state.lock().unwrap() {
            TunnelState::Connecting(_) => {
                tunnel::TunnelState::Connecting
            }, 
            TunnelState::PreActive(pre_active) => {
                tunnel::TunnelState::Active(pre_active.remote_timestamp)
            },
            TunnelState::Active(active) => {
                tunnel::TunnelState::Active(active.remote_timestamp)
            }, 
            TunnelState::Dead => {
                tunnel::TunnelState::Dead
            }
        }
    } 

    fn send_package(&self, package: DynamicPackage) -> Result<(), BuckyError> {
        if package.cmd_code() == PackageCmdCode::SessionData {
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, "session data should not send from tcp tunnel"));
        }
        let (signal_writer, to_connect) = {
            match &*self.0.state.lock().unwrap() {
                TunnelState::PreActive(pre_active) => {
                    Ok((pre_active.signal_writer.clone(), true))
                }, 
                TunnelState::Active(active) => {
                    Ok((active.signal_writer.clone(), false))
                },
                _ => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel not active"))
                }
            }
        }?;
        let _ = signal_writer.try_send(SignalElem::Package(PackageElem::Package(package)));
        if to_connect {
            let _ = self.connect();
        }
        Ok(())
    }

    fn raw_data_header_len(&self) -> usize {
        tcp::PackageInterface::raw_header_data_len()
    }

    fn send_raw_data(&self, data: &mut [u8]) -> Result<usize, BuckyError> {
        let (signal_writer, to_connect) = {
            match &*self.0.state.lock().unwrap() {
                TunnelState::PreActive(pre_active) => {
                    Ok((pre_active.signal_writer.clone(), true))
                }, 
                TunnelState::Active(active) => {
                    Ok((active.signal_writer.clone(), false))
                },
                _ => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel not active"))
                }
            }
        }?;
        let len = data.len();
        let _ = signal_writer.try_send(SignalElem::Package(PackageElem::RawData(Vec::from(data))));
        if to_connect {
            let _ = self.connect();
        }
        Ok(len)
    }

    fn ptr_eq(&self, other: &tunnel::DynamicTunnel) -> bool {
        *self.local() == *other.as_ref().local() 
        && *self.remote() == *other.as_ref().remote()
        && Arc::ptr_eq(&self.0, &other.clone_as_tunnel::<Tunnel>().0)
    }

    fn retain_keeper(&self) {
        info!("{} retain keeper", self);
        if 0 != self.0.keeper_count.fetch_add(1, Ordering::SeqCst) {
            return ;
        }
        let owner = {
            let state = &mut *self.0.state.lock().unwrap();
            match state {
                TunnelState::Connecting(_) => None, 
                TunnelState::PreActive(pre_active) => Some(pre_active.owner.clone()), 
                TunnelState::Active(_) => None, 
                TunnelState::Dead => None
            }
        };
        if owner.is_none() {
            return ;
        }

        let owner = owner.unwrap();
        let retain_connect_timestamp = bucky_time_now();
        self.0.retain_connect_timestamp.store(retain_connect_timestamp, Ordering::SeqCst);
        let tunnel = self.clone();
        task::spawn(async move {
            let _ = future::timeout(owner.config().tcp.retain_connect_delay, future::pending::<()>()).await;
            tunnel.retain_connect(retain_connect_timestamp, owner.config().tcp.ping_interval, owner.config().tcp.ping_timeout).await;
        });
    }

    fn release_keeper(&self) {
        info!("{} release keeper", self);
        self.0.keeper_count.fetch_add(-1, Ordering::SeqCst);
    }

    fn reset(&self) {
        info!("{} reset to Dead", self);
        if let Some(dead_waiters) = {
            let state = &mut *self.0.state.lock().unwrap();
            match state {
                TunnelState::Active(active) => {
                    let mut dead_waiters = StateWaiter::new();
                    std::mem::swap(&mut dead_waiters, &mut active.dead_waiters);
                    *state = TunnelState::Dead;
                    Some(dead_waiters)
                }, 
                _ => None
            }
        } {
            dead_waiters.wake();
        }
    }

    fn mark_dead(&self, former_state: tunnel::TunnelState) {
        let notify = match &former_state {
            tunnel::TunnelState::Connecting => {
                let state = &mut *self.0.state.lock().unwrap();
                match state {
                    TunnelState::Connecting(connecting) => {
                        info!("{} Connecting=>Dead", self);
                        let owner = connecting.owner.clone();
                        *state = TunnelState::Dead;
                        Some((owner, tunnel::TunnelState::Dead, None))
                    }, 
                    _ => {
                        None
                    }
                }
            }, 
            tunnel::TunnelState::Active(remote_timestamp) => {
                let remote_timestamp = *remote_timestamp;
                let state = &mut *self.0.state.lock().unwrap();
                match state {
                    TunnelState::Active(active) => {
                        let owner = active.owner.clone();
                        if active.remote_timestamp == remote_timestamp {
                            info!("{} Active({})=>Dead for active by {}", self, active.remote_timestamp, remote_timestamp);
                            let mut dead_waiters = StateWaiter::new();
                            std::mem::swap(&mut dead_waiters, &mut active.dead_waiters);
                            *state = TunnelState::Dead;
                            Some((owner, tunnel::TunnelState::Dead, Some(dead_waiters)))
                        } else {
                            None
                        }
                    }, 
                    _ => {
                        None
                    }
                }
            }, 
            tunnel::TunnelState::Dead => None
        };

        if let Some((owner, new_state, dead_waiters)) = notify {
            if let Some(dead_waiters) = dead_waiters {
                dead_waiters.wake();
            }
            owner.sync_tunnel_state(&DynamicTunnel::new(self.clone()), former_state, new_state);
        }
    }
}

impl OnPackage<PingTunnel> for Tunnel {
    fn on_package(&self, ping: &PingTunnel, _context: Option<()>) -> Result<OnPackageResult, BuckyError> {
        let ping_resp = PingTunnelResp {
            ack_package_id: ping.package_id,
            send_time: bucky_time_now(),
            recv_data: 0,
        };
        let _ = tunnel::Tunnel::send_package(self, DynamicPackage::from(ping_resp));
        Ok(OnPackageResult::Handled)
    }
}

impl OnPackage<PingTunnelResp> for Tunnel {
    fn on_package(&self, _pkg: &PingTunnelResp, _context: Option<()>) -> Result<OnPackageResult, BuckyError> {
        // do nothing
        Ok(OnPackageResult::Handled)
    }
}

impl OnTcpInterface for Tunnel {
    fn on_tcp_interface(&self, interface: tcp::AcceptInterface, first_box: PackageBox) -> Result<OnPackageResult, BuckyError> {
        assert_eq!(self.is_reverse(), true);
        assert_eq!(first_box.packages_no_exchange().len(), 1);
        let first_package = &first_box.packages_no_exchange()[0];
        if first_package.cmd_code() == PackageCmdCode::SynTunnel {
            let syn_tunnel: &SynTunnel = first_package.as_ref();
            let remote_timestamp = syn_tunnel.from_device_desc.body().as_ref().unwrap().update_time();
            let (owner, ret) = {
                let state = &mut *self.0.state.lock().unwrap();
                match state {
                    TunnelState::Connecting(connecting) => {
                        info!("{} accept interface {} in connecting", self, interface);
                        (Some(connecting.owner.clone()), ACK_TUNNEL_RESULT_OK)
                    }, 
                    TunnelState::PreActive(pre_active) => {
                        info!("{} accept interface {} in PreActive", self, interface);
                        (Some(pre_active.owner.clone()), ACK_TUNNEL_RESULT_OK)
                    }, 
                    TunnelState::Active(active) => {
                        if active.remote_timestamp < remote_timestamp {
                            info!("{} accept interface {} for active remote timestamp update from {} to {}", self, interface, active.remote_timestamp, remote_timestamp);
                            (Some(active.owner.clone()), ACK_TUNNEL_RESULT_OK)    
                        } else if active.syn_seq < syn_tunnel.sequence {
                            info!("{} accept interface {} for active sequence update from {:?} to {:?}", self, interface, active.syn_seq, syn_tunnel.sequence);
                            (Some(active.owner.clone()), ACK_TUNNEL_RESULT_OK)
                        } else {
                            info!("{} refuse interface {} for already active", self, interface);
                            (Some(active.owner.clone()), ACK_TUNNEL_RESULT_REFUSED)
                        }
                    }, 
                    TunnelState::Dead => {
                        info!("{} refuse interface {} for dead", self, interface);
                        (None, ACK_TUNNEL_RESULT_REFUSED)
                    }
                }
            };
            if let Some(owner) = owner {
                owner.on_package(syn_tunnel, None)?;
                let ack_tunnel = AckTunnel {
                    protocol_version: owner.protocol_version(), 
                    stack_version: owner.stack_version(),  
                    sequence: syn_tunnel.sequence,
                    result: ret,
                    send_time: bucky_time_now(),
                    mtu: udp::MTU as u16,
                    to_device_desc: owner.stack().local().clone(),
                };
                let tunnel = self.clone();
                task::spawn(async move {
                    let syn_seq = ack_tunnel.sequence;
                    let confirm_ret = interface.confirm_accept(vec![DynamicPackage::from(ack_tunnel)]).await;
                    if ret == ACK_TUNNEL_RESULT_OK {
                        tunnel.active_with_interface(confirm_ret.map(|_| (interface.into(), remote_timestamp, syn_seq)));
                    } else {
                        // do nothing
                    }
                });
            }
            Ok(OnPackageResult::Handled)
        } else if first_package.cmd_code() == PackageCmdCode::TcpSynConnection {
            let syn_stream: &TcpSynConnection = first_package.as_ref();
            let owner = self.pre_active(syn_stream.from_device_desc.body().as_ref().unwrap().update_time())?;
            owner.on_package(syn_stream, interface)
        } else if first_package.cmd_code() == PackageCmdCode::TcpAckConnection {
            let ack_stream: &TcpAckConnection = first_package.as_ref();
            let owner = self.pre_active(ack_stream.to_device_desc.body().as_ref().unwrap().update_time())?;
            owner.on_package(ack_stream, interface)
        } else {
            unreachable!()
        }
    }
}


