use log::*;
use std::{
    ops::Deref, 
    time::Duration,
    path::PathBuf
    // sync::{atomic::{AtomicU64, Ordering}}
};
use async_std::{
    sync::{Arc, Weak}, 
    task, 
    future, 
};
use cyfs_base::*;

use crate::{
    types::*,
    cc::{self},
    datagram::{self, DatagramManager},
    finder::*,
    history::keystore,
    interface::{
        self, 
        NetManager, 
        tcp::{self, OnTcpInterface},
        udp::{self, OnUdpPackageBox, OnUdpRawData, UdpPackageBox},
    },
    protocol::{*, v0::*},
    sn::{
        self,
        client::{PingClientCalledEvent, PingClients},
    },
    stream::{self, StreamManager},
    tunnel::{self, TunnelManager},
    pn::client::ProxyManager,
    ndn::{self, HistorySpeedConfig, NdnStack, ChunkReader, NdnEventHandler, RawCacheConfig }, 
    debug::{self, DebugStub}
};


struct StackLazyComponents {
    sn_client: sn::client::ClientManager,
    tunnel_manager: TunnelManager,
    stream_manager: StreamManager,
    datagram_manager: DatagramManager,
    proxy_manager: ProxyManager, 
    debug_stub: Option<DebugStub>
}

#[derive(Clone)]
pub struct StackConfig {
    pub statistic_interval: Duration, 
    pub device_cache: DeviceCacheConfig, 
    pub keystore: keystore::Config,
    pub interface: interface::Config, 
    pub sn_client: sn::client::Config,
    pub tunnel: tunnel::Config,
    pub stream: stream::Config,
    pub datagram: datagram::Config,
    pub ndn: ndn::Config, 
    pub debug: Option<debug::Config>
}

impl StackConfig {
    pub fn new(_isolate: &str) -> Self {
        Self {
            statistic_interval: Duration::from_secs(60),
            keystore: keystore::Config {
                active_time: Duration::from_secs(300),
                capacity: 10000,
            }, 
            device_cache: DeviceCacheConfig {
                expire: Duration::from_secs(5 * 60),
                capacity: 1024 * 1024
            }, 
            interface: interface::Config {
                udp: interface::udp::Config {
                    sn_only: false, 
                    sim_loss_rate: 0, 
                    recv_buffer: 52428800
                }
            },
            sn_client: sn::client::Config {
                atomic_interval: Duration::from_millis(100),
                ping: sn::client::ping::PingConfig {
                    interval: Duration::from_secs(25), 
                    udp: sn::client::ping::udp::Config {
                        resend_interval: Duration::from_millis(500),
                        resend_timeout: Duration::from_secs(5),
                    }
                }, 
                call: sn::client::call::CallConfig {
                    timeout: Duration::from_secs(5), 
                    first_try_timeout: Duration::from_secs(2), 
                    udp: sn::client::call::udp::Config {
                        resend_interval: Duration::from_millis(500),
                    }
                }
            },
            tunnel: tunnel::Config {
                retain_timeout: Duration::from_secs(60),
                retry_sn_timeout: Duration::from_secs(2), 
                connect_timeout: Duration::from_secs(5),
                tcp: tunnel::tcp::Config {
                    connect_timeout: Duration::from_secs(5), 
                    confirm_timeout: Duration::from_secs(5), 
                    accept_timeout: Duration::from_secs(5), 
                    retain_connect_delay: Duration::from_secs(5), 
                    ping_interval: Duration::from_secs(30), 
                    ping_timeout: Duration::from_secs(60), 
                    package_buffer: 100, 
                    piece_buffer: 1000, 
                    piece_interval: Duration::from_millis(10), 
                }, 
                udp: tunnel::udp::Config {
                    holepunch_interval: Duration::from_millis(200),
                    connect_timeout: Duration::from_secs(5),
                    ping_interval: Duration::from_secs(30),
                    ping_timeout: Duration::from_secs(60 * 3),
                },
            },
            stream: stream::Config {
                listener: stream::listener::Config { backlog: 100 },
                stream: stream::container::Config {
                    nagle: Duration::from_millis(0),
                    recv_buffer: 1024 * 1024,
                    recv_timeout: Duration::from_millis(200),
                    drain: 0.5,
                    send_buffer: 1024 * 512, // 这个值不能小于下边的max_record
                    retry_sn_timeout: Duration::from_secs(2), 
                    connect_timeout: Duration::from_secs(5),
                    tcp: stream::tcp::Config {
                        min_record: 1024,
                        max_record: 2048,
                    },
                    package: stream::package::Config {
                        connect_resend_interval: Duration::from_millis(100),
                        atomic_interval: Duration::from_millis(10),
                        break_overtime: Duration::from_secs(60),
                        msl: Duration::from_secs(60), 
                        cc: cc::Config {
                            init_rto: Duration::from_secs(1),
                            min_rto: Duration::from_millis(200),
                            cc_impl: cc::ImplConfig::BBR(Default::default()),
                        },
                    },
                },
            },
            datagram: datagram::Config {
                min_random_vport: 32767,
                max_random_vport: 65535,
                max_try_random_vport_times: 5,
                piece_cache_duration: Duration::from_millis(1000),
                recv_cache_count: 16,
                expired_tick_sec: 10,
                fragment_cache_size: 100 *1024*1024,
                fragment_expired_us: 30 *1000*1000,
            },
            ndn: ndn::Config {
                atomic_interval: Duration::from_millis(10), 
                schedule_interval: Duration::from_secs(1), 
                channel: ndn::channel::Config {
                    reserve_timeout: Duration::from_secs(60), 
                    resend_interval: Duration::from_millis(500), 
                    resend_timeout: Duration::from_secs(5), 
                    block_interval: Duration::from_secs(2), 
                    msl: Duration::from_secs(60), 
                    udp: ndn::channel::tunnel::udp::Config {
                        no_resp_loss_count: 3, 
                        break_loss_count: 10, 
                        cc: cc::Config {
                            init_rto: Duration::from_secs(1),
                            min_rto: Duration::from_millis(200),
                            cc_impl: cc::ImplConfig::Ledbat(Default::default()),
                        }
                    }, 
                    history_speed: HistorySpeedConfig {
                        attenuation: 0.5, 
                        expire: Duration::from_secs(20),  
                        atomic: Duration::from_secs(1)
                    }
                }, 
                chunk: ndn::chunk::Config{
                    raw_caches: RawCacheConfig {
                        mem_capacity: 1024 * 1024 * 1024, 
                        tmp_dir: PathBuf::new()
                    }
                }
            }, 
            debug: None
        }
    }
}

pub struct StackImpl {
    config: StackConfig,
    local_device_id: DeviceId,
    local_const: DeviceDesc,
    id_generator: IncreaseIdGenerator,
    keystore: keystore::Keystore,
    device_cache: DeviceCache,
    net_manager: NetManager,
    lazy_components: Option<StackLazyComponents>, 
    ndn: Option<NdnStack>, 
}

pub struct StackOpenParams {
    pub config: StackConfig, 
    pub tcp_port_mapping: Option<Vec<(Endpoint, u16)>>, 
    pub known_sn: Option<Vec<Device>>,
    pub known_device: Option<Vec<Device>>, 
    pub active_pn: Option<Vec<Device>>, 
    pub passive_pn: Option<Vec<Device>>, 

    pub outer_cache: Option<Box<dyn OuterDeviceCache>>,
    pub chunk_store: Option<Box<dyn ChunkReader>>, 

    pub ndn_event: Option<Box<dyn NdnEventHandler>>,
}

impl StackOpenParams {
    pub fn new(isolate: &str) -> Self {
        Self {
            config: StackConfig::new(isolate), 
            tcp_port_mapping: None, 
            known_sn: None, 
            known_device: None, 
            active_pn: None, 
            passive_pn: None,
            outer_cache: None,
            chunk_store: None, 
            ndn_event: None,
        }
    }
}

#[derive(Clone)]
pub struct Stack(Arc<StackImpl>);
pub type WeakStack = Weak<StackImpl>;

impl Stack {
    pub async fn open(
        local_device: Device,
        local_secret: PrivateKey,
        params: StackOpenParams
    ) -> Result<StackGuard, BuckyError> {
        let local_device_id = local_device.desc().device_id();
        
        let mut params = params;
        let mut tcp_port_mapping = None;
        std::mem::swap(&mut tcp_port_mapping, &mut params.tcp_port_mapping);
        
        let net_manager =
            NetManager::open(
                local_device_id.clone(), 
                &params.config.interface, 
                &local_device.connect_info().endpoints(), 
                tcp_port_mapping)?;
        let net_listener = net_manager.listener();

        let signer = RsaCPUObjectSigner::new(
            local_device.desc().public_key().clone(),
            local_secret.clone(),
        );

        let mut passive_pn = vec![];
        if params.passive_pn.is_some() {
            std::mem::swap(&mut passive_pn, params.passive_pn.as_mut().unwrap());
        }

        let init_local_device = {
            let mut device = local_device.clone();
            let device_endpoints = device.mut_connect_info().mut_endpoints();
            device_endpoints.clear();
            let bound_endpoints = net_manager.listener().endpoints();
            for ep in bound_endpoints {
                device_endpoints.push(ep);
            }
            
            let passive_pn_list = device.mut_connect_info().mut_passive_pn_list();
            for pn in passive_pn.iter().map(|d| d.desc().device_id()) {
                passive_pn_list.push(pn);
            }

            device
                .body_mut()
                .as_mut()
                .unwrap()
                .increase_update_time(bucky_time_now());
            sign_and_set_named_object_body(&signer, &mut device, &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF))
                .await
                .map(|_| device)
        }?;

        let key_store = keystore::Keystore::new(
            local_secret,
            local_device.desc().clone(),
            signer,
            params.config.keystore.clone(),
        );

        let mut outer_cache = None;
        std::mem::swap(&mut outer_cache, &mut params.outer_cache);

        let stack = Self(Arc::new(StackImpl {
            config: params.config.clone(),
            local_device_id,
            local_const: local_device.desc().clone(),
            id_generator: IncreaseIdGenerator::new(),
            keystore: key_store,
            device_cache: DeviceCache::new(&params.config.device_cache, outer_cache),
            net_manager,
            lazy_components: None, 
            ndn: None
        }));

        let datagram_manager = DatagramManager::new(stack.to_weak());

        let proxy_manager = ProxyManager::new(stack.to_weak());

        let mut active_pn = vec![];
        if params.active_pn.is_some() {
            std::mem::swap(&mut active_pn, params.active_pn.as_mut().unwrap());
        }
        for pn in active_pn {
            proxy_manager.add_active_proxy(&pn);
        }

        for pn in passive_pn {
            proxy_manager.add_passive_proxy(&pn);
        }

        let debug_stub = if stack.config().debug.is_some() {
            Some(DebugStub::open(stack.to_weak(), stack.config().debug.as_ref().unwrap().chunk_store.clone()).await?)
        } else {
            None
        };

        {
            let components = StackLazyComponents {
                sn_client: sn::client::ClientManager::create(stack.to_weak(), net_listener, init_local_device.clone()),
                tunnel_manager: TunnelManager::new(stack.to_weak()),
                stream_manager: StreamManager::new(stack.to_weak()),
                datagram_manager, 
                proxy_manager, 
                debug_stub: debug_stub.clone()
            };
            
            let stack_impl = unsafe { &mut *(Arc::as_ptr(&stack.0) as *mut StackImpl) };
            stack_impl.lazy_components = Some(components);

            let mut chunk_store = None;
            std::mem::swap(&mut chunk_store, &mut params.chunk_store);

            let mut ndn_event = None;
            std::mem::swap(&mut ndn_event, &mut params.ndn_event);
    
            let ndn = NdnStack::open(stack.to_weak(), chunk_store, ndn_event);
            let stack_impl = unsafe { &mut *(Arc::as_ptr(&stack.0) as *mut StackImpl) };
            stack_impl.ndn = Some(ndn);

        }
        

        let mut known_device = vec![];
        if params.known_device.is_some() {
            std::mem::swap(&mut known_device, params.known_device.as_mut().unwrap());
        }
        for device in known_device {
            stack
                .device_cache()
                .add_static(&device.desc().device_id(), &device);
        }

        let net_listener = stack.net_manager().listener();
        net_listener.start(stack.to_weak());
        
        let mut known_sn = vec![];
        if params.known_sn.is_some() {
            std::mem::swap(&mut known_sn, params.known_sn.as_mut().unwrap());
        }
        stack.reset_known_sn(known_sn.clone());
        stack.ndn().start();

        if let Some(debug_stub) = debug_stub {
            debug_stub.listen();
        }
        
        let arc_stack = stack.clone();
        task::spawn(async move {
            loop {
                info!("{} statistic: {}, {}, {}, {}", 
                    arc_stack, 
                    arc_stack.tunnel_manager().on_statistic(), 
                    arc_stack.stream_manager().on_statistic(),
                    arc_stack.ndn().channel_manager().on_statistic(), 
                    arc_stack.ndn().chunk_manager().on_statistic()
                );
                let _ = future::timeout(arc_stack.config().statistic_interval, future::pending::<()>()).await;
            }
        });

        info!("{}: opened, version 0.5.4", stack); 
        Ok(StackGuard::from(stack))
    }

    pub fn to_weak(&self) -> WeakStack {
        Arc::downgrade(&self.0)
    }

    pub fn id_generator(&self) -> &IncreaseIdGenerator {
        &self.0.id_generator
    }

    pub fn keystore(&self) -> &keystore::Keystore {
        &self.0.keystore
    }

    pub fn net_manager(&self) -> &NetManager {
        &self.0.net_manager
    }

    pub fn device_cache(&self) -> &DeviceCache {
        &self.0.device_cache
    }

    pub fn config(&self) -> &StackConfig {
        &self.0.config
    }
    pub fn tunnel_manager(&self) -> &TunnelManager {
        &self.0.lazy_components.as_ref().unwrap().tunnel_manager
    }
    pub fn stream_manager(&self) -> &StreamManager {
        &self.0.lazy_components.as_ref().unwrap().stream_manager
    }

    pub fn datagram_manager(&self) -> &DatagramManager {
        &self.0.lazy_components.as_ref().unwrap().datagram_manager
    }

    pub fn proxy_manager(&self) -> &ProxyManager {
        &self.0.lazy_components.as_ref().unwrap().proxy_manager
    }

    pub fn local_device_id(&self) -> &DeviceId {
        &self.0.local_device_id
    }

    pub fn local_const(&self) -> &DeviceDesc {
        &self.0.local_const
    }

    pub fn sn_client(&self) -> &sn::client::ClientManager {
        &self.0.lazy_components.as_ref().unwrap().sn_client
    }

    pub fn ndn(&self) -> &NdnStack {
        &self.0.ndn.as_ref().unwrap()
    }

    pub fn close(&self) {
        //unimplemented!()
    }

    pub fn reset_sn_list(&self, sn_list: Vec<Device>) -> PingClients {
        let sn_id_list: Vec<DeviceId> = sn_list.iter().map(|sn| sn.desc().device_id()).collect();
        info!("{} reset_sn_list {:?}", self, sn_id_list);
        self.sn_client().reset_sn_list(sn_list)
    }

    pub fn reset_known_sn(&self, sn_list: Vec<Device>) {
        let sn_id_list: Vec<DeviceId> = sn_list.iter().map(|sn| sn.desc().device_id()).collect();
        info!("{} reset_known_sn_list {:?}", self, sn_id_list);
        for (id, sn) in sn_id_list.iter().zip(sn_list.iter()) {
            self.device_cache().add_static(id, sn);
        }
        self.sn_client().cache().reset_known_sn(&sn_id_list);
    }

    pub async fn reset_endpoints(&self, endpoints: &Vec<Endpoint>) -> PingClients {
        info!("{} reset {:?}", self, endpoints);
        let listener = self.net_manager().reset(endpoints.as_slice());
        
        let mut local = self.sn_client().ping().default_local();
        let device_endpoints = local.mut_connect_info().mut_endpoints();
        device_endpoints.clear();
        let bound_endpoints = listener.endpoints();
        for ep in bound_endpoints {
            device_endpoints.push(ep);
        }
        local
            .body_mut()
            .as_mut()
            .unwrap()
            .increase_update_time(bucky_time_now());
        let _ = sign_and_set_named_object_body(
            self.keystore().signer(),
            &mut local,
            &SignatureSource::RefIndex(0),
        )
        .await;
        self.tunnel_manager().reset();

        self.sn_client().reset_endpoints(listener.clone(), local)
    }
}

impl std::fmt::Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BdtStack{{local:{}}}", self.local_device_id())
    }
}

impl From<&WeakStack> for Stack {
    fn from(w: &WeakStack) -> Self {
        Self(w.upgrade().unwrap())
    }
}

impl OnUdpPackageBox for Stack {
    fn on_udp_package_box(&self, package_box: UdpPackageBox) -> Result<(), BuckyError> {
        trace!("{} on_udp_package_box", self.local_device_id().as_ref());
        //FIXME: 用sequence 和 send time 过滤
        if package_box.as_ref().has_exchange() {
            // let exchange: &Exchange = package_box.as_ref().packages()[0].as_ref();
            self.keystore().add_key(
                package_box.as_ref().key(),
                package_box.as_ref().remote()
            );
        }
        if package_box.as_ref().is_tunnel() {
            self.tunnel_manager().on_udp_package_box(package_box)
        } else if package_box.as_ref().is_sn() {
            self.sn_client().on_udp_package_box(package_box)
        } else if package_box.as_ref().is_tcp_stream() {
            self.tunnel_manager().on_udp_package_box(package_box)
        } else if package_box.as_ref().is_proxy() {
            self.proxy_manager().on_udp_package_box(package_box)
        } else {
            unreachable!()
        }
    }
}

impl OnUdpRawData<(udp::Interface, DeviceId, MixAesKey, Endpoint)> for Stack {
    fn on_udp_raw_data(
        &self,
        data: &[u8],
        context: (udp::Interface, DeviceId, MixAesKey, Endpoint),
    ) -> Result<(), BuckyError> {
        self.tunnel_manager().on_udp_raw_data(data, context)
    }
}

impl OnTcpInterface for Stack {
    fn on_tcp_interface(
        &self,
        interface: tcp::AcceptInterface,
        first_box: PackageBox,
    ) -> Result<OnPackageResult, BuckyError> {
        //FIXME: 用sequence 和 send time 过滤
        if first_box.has_exchange() {
            // let exchange: &Exchange = first_box.packages()[0].as_ref();
            self.keystore()
                .add_key(first_box.key(), first_box.remote());
        }
        if first_box.is_tunnel() {
            self.tunnel_manager().on_tcp_interface(interface, first_box)
        } else if first_box.is_sn() {
            unreachable!()
        } else if first_box.is_tcp_stream() {
            self.tunnel_manager().on_tcp_interface(interface, first_box)
        } else {
            unreachable!()
        }
    }
}


impl PingClientCalledEvent for Stack {
    fn on_called(&self, called: &SnCalled, _: ()) -> Result<(), BuckyError> {
        if called.payload.len() == 0 {
            warn!("{} ignore called for no payload.", self.local_device_id());
            return Ok(());
        }
        use udp::*;
        let mut crypto_buf = vec![0u8; called.payload.as_ref().len()];
        let ctx = PackageBoxDecodeContext::new_copy(crypto_buf.as_mut(), self.keystore());
        let caller_box = PackageBox::raw_decode_with_context(
            called.payload.as_ref(),
            (ctx, Some(called.into())),
        ).map(|(package_box, _)| package_box)
        .map_err(|err| {
            error!("{} ignore decode payload failed, err={}.", self.local_device_id(), err);
            err
        })?;
        if caller_box.has_exchange() {
            // let exchange: &Exchange = caller_box.packages()[0].as_ref();
            self.keystore().add_key(caller_box.key(), caller_box.remote());
        }
        self.tunnel_manager().on_called(called, caller_box)
    }
}

struct StackGuardImpl(Stack);

impl Drop for StackGuardImpl {
    fn drop(&mut self) {
        self.0.close();
    }
}

#[derive(Clone)]
pub struct StackGuard(Arc<StackGuardImpl>);

impl From<Stack> for StackGuard {
    fn from(stack: Stack) -> Self {
        Self(Arc::new(StackGuardImpl(stack)))
    }
}

impl Deref for StackGuard {
    type Target = Stack;
    fn deref(&self) -> &Stack {
        &(*self.0).0
    }
}
