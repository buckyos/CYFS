use log::*;
use std::{
    time::{Duration}, 
    io::{ErrorKind}, 
    net::SocketAddr,
    future::Future, 
    pin::Pin, 
    sync::Arc, 
    // thread
};
use async_std::{
    net::{UdpSocket, TcpListener, Shutdown}, 
    task::{self, Context, Poll}, 
};
use futures::{
    executor::ThreadPool
};
use cyfs_base::*;
use crate::{
    protocol::*,
    history::keystore::Keystore, 
    interface::{
        udp::{MTU, PackageBoxDecodeContext, PackageBoxEncodeContext},
        tcp::{AcceptInterface, PackageInterface},
    },
};
use super::SnService;

pub struct NetListener {
    thread_pool: ThreadPool, 
    close_notifier: async_std::channel::Sender<()>
}

impl NetListener {
    async fn process_pkg(service: &SnService, pkg_box: PackageBox, resp_sender: MessageSender) {
        // 通知处理
        let mut cmd_pkg = pkg_box.packages().get(0);
        if let Some(first_pkg) = pkg_box.packages().get(0) {
            if let PackageCmdCode::Exchange = first_pkg.cmd_code() {
                let exchg = first_pkg.as_any().downcast_ref::<Exchange>();
                if let Some(exchg) = exchg {
                    if !exchg.verify(pkg_box.key()).await {
                        warn!("exchange sign-verify failed, from: {:?}.", resp_sender.remote());
                        return;
                    }
                } else {
                    warn!("fetch exchange failed, from: {:?}.", resp_sender.remote());
                    return;
                }
                cmd_pkg = pkg_box.packages().get(1);
            };
        }

        if let Some(cmd_pkg) = cmd_pkg {
            if let PackageCmdCode::SnPing = cmd_pkg.cmd_code() {
                let peer_info = if let Some(ping_req) = cmd_pkg.as_any().downcast_ref::<SnPing>() {
                    ping_req.peer_info.as_ref()
                } else {
                    None
                };
                if let Some(peer_info) = peer_info {
                    if let Some(sigs) = peer_info.signs().body_signs() {
                        if let Some(sig) = sigs.get(0) {
                            match verify_object_body_sign(&RsaCPUObjectVerifier::new(peer_info.desc().public_key().clone()), peer_info, sig).await {
                                Ok(is_ok) => {
                                    if !is_ok {
                                        log::warn!("sn-ping verify but unmatch, from {:?}", resp_sender.remote());
                                        return;
                                    }
                                },
                                Err(e) => {
                                    log::warn!("sn-ping verify failed, from {:?}, {}", resp_sender.remote(), e);
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            service.handle(pkg_box, resp_sender);
        }
    }

    async fn run_udp(service: SnService, udp_listener: UdpListener) {
        loop {
            let (pkg, sender) = match udp_listener.recv().await {
                Ok(ret) => {
                    ret
                }
                Err(_) => {
                    error!("udp listener recv error! addr={:?}", udp_listener.0.addr);
                    async_std::task::sleep(std::time::Duration::from_secs(10)).await;
                    continue;
                }
            };
            
        
            Self::process_pkg(&service, pkg, sender).await;
        }
    }

    async fn run_tcp(service: SnService, tcp_listener: TcpAcceptor) {
        loop {
            let (pkg, sender) = match tcp_listener.accept().await {
                Ok(ret) => {
                    ret
                }
                Err(_) => {
                    error!("tcp listener recv error! addr={:?}", tcp_listener.addr);
                    async_std::task::sleep(std::time::Duration::from_secs(10)).await;
                    continue;
                }
            };
            
            Self::process_pkg(&service, pkg, sender).await;
        }
    }

    /*
    async fn listen_process(
        udp_listeners: Vec<UdpListener>, 
        tcp_listeners: Vec<TcpAcceptor>, 
        service: SnService, 
        close_waiter: async_std::channel::Receiver<()>) {
        
        let mut udp_listen_futures: Vec<Pin<Box<dyn Future<Output = BuckyResult<(PackageBox, MessageSender)>> + Send>>> = Vec::with_capacity(udp_listeners.len());
        for udp in udp_listeners.as_slice() {
            udp_listen_futures.push(Box::pin(udp.recv()));
        }
        let mut tcp_listen_futures: Vec<Pin<Box<dyn Future<Output = BuckyResult<(PackageBox, MessageSender)>> + Send>>> = Vec::with_capacity(tcp_listeners.len());
        for tcp in tcp_listeners.as_slice() {
            tcp_listen_futures.push(Box::pin(tcp.accept()));
        }

        let mut close_wait_future_container = Some(Box::pin(close_waiter.recv()));

        let mut active_udp_count = udp_listeners.len();
        let mut active_tcp_count = tcp_listeners.len();

        if active_udp_count == 0 {
            udp_listen_futures.push(Box::pin(RecvPending::new()));
        }
        if active_tcp_count == 0 {
            tcp_listen_futures.push(Box::pin(RecvPending::new()));
        }
        let mut udp_listen_selector = Some(futures::future::select_all(udp_listen_futures));
        let mut tcp_listen_selector = Some(futures::future::select_all(tcp_listen_futures));

        loop {
            let close_wait_future = close_wait_future_container.take().unwrap();
            let udp_listen_future = udp_listen_selector.take().unwrap();
            let tcp_listen_future = tcp_listen_selector.take().unwrap();

            let result = match futures::future::select(close_wait_future,
                                                        futures::future::select(udp_listen_future, tcp_listen_future)
            ).await {
                Either::Left(_) => {
                    log::info!("net-listener will stop.");
                    break;
                },
                Either::Right((listen_result, close_wait_future)) => {
                    close_wait_future_container = Some(close_wait_future);
                    match listen_result {
                        Either::Left(((result, index, mut remain_futures), tcp_futures)) => {
                            let new_future: Pin<Box<dyn Future<Output = BuckyResult<(PackageBox, MessageSender)>> + Send>> = match result.as_ref() {
                                Ok(_) => {
                                    Box::pin(udp_listeners.get(index).unwrap().recv())
                                }
                                Err(_) => {
                                    active_udp_count -= 1;
                                    if active_udp_count == 0 {
                                        log::error!("udp listeners break all");
                                    }
                                    Box::pin(RecvPending::new())
                                }
                            };

                            // FIXME:依赖于select_all实现(把最后一个Pending状态Future填充到移除的Ready状态Future)
                            remain_futures.push(new_future);
                            let last_index = remain_futures.len() - 1;
                            if last_index > 0 {
                                remain_futures.swap(index, last_index);
                            }

                            udp_listen_selector = Some(futures::future::select_all(remain_futures));
                            tcp_listen_selector = Some(tcp_futures);
                            result
                        },
                        Either::Right(((result, index, mut remain_futures), udp_futures)) => {
                            let new_future: Pin<Box<dyn Future<Output = BuckyResult<(PackageBox, MessageSender)>> + Send>> = match result.as_ref() {
                                Ok(_) => {
                                    Box::pin(tcp_listeners.get(index).unwrap().accept())
                                }
                                Err(_) => {
                                    active_tcp_count -= 1;
                                    if active_tcp_count == 0 {
                                        log::error!("tcp listeners break all");
                                    }
                                    Box::pin(RecvPending::new())
                                }
                            };

                            // FIXME:依赖于select_all实现(把最后一个Pending状态Future填充到移除的Ready状态Future)
                            remain_futures.push(new_future);
                            let last_index = remain_futures.len() - 1;
                            if last_index > 0 {
                                remain_futures.swap(index, last_index);
                            }

                            tcp_listen_selector = Some(futures::future::select_all(remain_futures));
                            udp_listen_selector = Some(udp_futures);
                            result
                        }
                    }
                }
            };

            if let Ok((pkg_box, resp_sender)) = result {
                // 通知处理
                let mut cmd_pkg = pkg_box.packages().get(0);
                if let Some(first_pkg) = pkg_box.packages().get(0) {
                    if let PackageCmdCode::Exchange = first_pkg.cmd_code() {
                        let exchg = first_pkg.as_any().downcast_ref::<Exchange>();
                        if let Some(exchg) = exchg {
                            if !exchg.verify(pkg_box.key()).await {
                                warn!("exchange sign-verify failed, from: {:?}.", resp_sender.remote());
                                return;
                            }
                        } else {
                            warn!("fetch exchange failed, from: {:?}.", resp_sender.remote());
                            return;
                        }
                        cmd_pkg = pkg_box.packages().get(1);
                    };
                }

                if let Some(cmd_pkg) = cmd_pkg {
                    if let PackageCmdCode::SnPing = cmd_pkg.cmd_code() {
                        let peer_info = if let Some(ping_req) = cmd_pkg.as_any().downcast_ref::<SnPing>() {
                            ping_req.peer_info.as_ref()
                        } else {
                            None
                        };
                        if let Some(peer_info) = peer_info {
                            if let Some(sigs) = peer_info.signs().body_signs() {
                                if let Some(sig) = sigs.get(0) {
                                    match verify_object_body_sign(&RsaCPUObjectVerifier::new(peer_info.desc().public_key().clone()), peer_info, sig).await {
                                        Ok(is_ok) if is_ok => {},
                                        _ => {
                                            log::warn!("sn-ping verify failed, from {:?}", resp_sender.remote());
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    service.handle(pkg_box, resp_sender);
                }
            }
        }
        // for listener in udp_listeners {
            //     listener.close();
            // }
            // for listener in tcp_listeners {
            //     listener.close();
            // }
    }
    */

    async fn bind(endpoints: &[Endpoint], key_store: &Keystore) -> (Vec<BuckyResult<UdpListener>>, Vec<BuckyResult<TcpAcceptor>>) {
        let mut udp_futures = vec![];
        let mut tcp_futures = vec![];

        if endpoints.len() == 0 {
            return (vec![], vec![]);
        }

        for endpoint in endpoints {
            match endpoint.protocol() {
                Protocol::Udp => {
                    udp_futures.push(UdpListener::bind(endpoint.addr().clone(), key_store.clone()));
                },
                Protocol::Tcp => {
                    tcp_futures.push(TcpAcceptor::bind(endpoint.addr().clone(), key_store.clone()));
                }
                Protocol::Unk => {
                    log::info!("sn-miner unknown listener.")
                }
            }
        }

        if udp_futures.is_empty() {
            futures::future::join(futures::future::ready(vec![]), futures::future::join_all(tcp_futures)).await
        } else if tcp_futures.is_empty() {
            futures::future::join(futures::future::join_all(udp_futures), futures::future::ready(vec![])).await
        } else {
            futures::future::join(futures::future::join_all(udp_futures), futures::future::join_all(tcp_futures)).await
        }
    }

    pub async fn listen(
        endpoints_v6: &[Endpoint], 
        endpoints_v4: &[Endpoint], 
        service: SnService) -> BuckyResult<(NetListener, usize, usize)> {
        
        let (mut udp_results, mut tcp_results) = Self::bind(endpoints_v6, service.key_store()).await;
        let (mut udp_results_v4, mut tcp_results_v4) = Self::bind(endpoints_v4, service.key_store()).await;

        udp_results.append(&mut udp_results_v4);
        tcp_results.append(&mut tcp_results_v4);

        assert!(!udp_results.is_empty());

        let mut udp_listeners = vec![];
        for r in udp_results {
            if let Ok(u) = r { udp_listeners.push(u); }
        }

        let mut tcp_listeners = vec![];
        for r in tcp_results {
            if let Ok(t) = r { tcp_listeners.push(t); }
        }

        let udp_count = udp_listeners.len();
        let tcp_count = tcp_listeners.len();
        let (close_notifier, _close_waiter) = async_std::channel::bounded(1);
        
        let pool_size = 4;
        let mut builder = ThreadPool::builder();
        let thread_pool = builder.pool_size(pool_size).create().unwrap();
        
        // 每个socket在多个任务处理，理想情况下每个线程处理一个udp+tcp
        for _ in 0..pool_size {
            for listener in &udp_listeners {
                let service = service.clone();
                let listener = listener.clone();
                thread_pool.spawn_ok(async move {
                    Self::run_udp(service, listener).await;
                });
            }

            for listener in &tcp_listeners {
                let service = service.clone();
                let listener = listener.clone();
                thread_pool.spawn_ok(async move {
                    Self::run_tcp(service, listener).await;
                });
            }
        }

        /*
        for _ in 0..pool_size {
            let udp_listeners = udp_listeners.clone();
            let tcp_listeners = tcp_listeners.clone();
            let service = service.clone();
            let close_waiter = close_waiter.clone();
            thread_pool.spawn_ok(async move {
                Self::listen_process(
                    udp_listeners, 
                    tcp_listeners, 
                    service, 
                    close_waiter).await;
            });
        }
        */

        Ok((NetListener {
            thread_pool, 
            close_notifier
        }, udp_count, tcp_count))
    }

    pub fn close(self) {
        // self.close_notifier.send(()).await;
    }
}

impl Drop for NetListener {
    fn drop(&mut self) {
        let notifier = self.close_notifier.clone();
        task::spawn(async move {
            let _ = notifier.send(()).await.map_err(|e|{
                log::error!("NetListener drop, notifier.send err: {}", e);
            });
        });
    }
}

struct UdpInterface {
    addr: SocketAddr,
    socket: UdpSocket,
    key_store: Keystore,
}

#[derive(Clone)]
struct UdpListener(Arc<UdpInterface>);

impl UdpListener {
    async fn bind(addr: SocketAddr, key_store: Keystore) -> BuckyResult<UdpListener> {
        let addr_str = addr.to_string();

        match UdpSocket::bind(addr.clone()).await {
            Err(e) => {
                warn!("udp({}) bind failed, err: {}", addr, e);
                Err(BuckyError::from(e))
            },
            Ok(socket) => {
                info!("udp({}) bind ok.", addr_str);
                #[cfg(windows)]
                    {
                        // 避免udp被对端reset
                        let r = cyfs_util::init_udp_socket(&socket);
                        if let Err(e) = r {
                            warn!("udp({}) connect reset disable failed, err is: {}", addr_str, e);
                        }
                    }
                Ok(UdpListener(Arc::new(UdpInterface {
                    addr,
                    socket,
                    key_store
                })))
            }
        }
    }

    async fn recv(&self) -> BuckyResult<(PackageBox, MessageSender)> {
        loop {
            let mut recv_buf = [0; MTU];
            let rr = self.0.socket.recv_from(&mut recv_buf).await;

            match rr {
                Ok((len, from)) => {
                    trace!("udp({}) recv {} bytes from {}", self.0.addr, len, from);
                    let recv = &mut recv_buf[..len];

                    let ctx = PackageBoxDecodeContext::new_inplace(recv.as_mut_ptr(), recv.len(), &self.0.key_store);
                    match PackageBox::raw_decode_with_context(recv, ctx) {
                        Ok((package_box, _)) => {
                            let resp_sender = MessageSender::Udp(UdpSender::new(self.0.clone(), package_box.remote().clone(), package_box.key().clone(), from));
                            break Ok((package_box, resp_sender))
                        },
                        Err(e) => {
                            warn!("udp({}) decode failed, len={}, from={}, e={}, first-u16: {}", self.0.addr, recv.len(), from, e, u16::raw_decode(recv).unwrap_or((0, recv)).0);
                        }
                    }
                },
                Err(e) => {
                    warn!("udp({}) recv failed({})", self.0.addr, e);
                    match e.kind() {
                        ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::AlreadyExists | ErrorKind::TimedOut => continue,
                        _ => {
                            warn!("udp({}) recv fatal error({}). will stop.", self.0.addr, e);
                            break Err(BuckyError::from(e));
                        },
                    }
                }
            }
        }
    }

    fn close(self) {
        // let _ = self.0.socket.close().await;
    }
}

#[derive(Clone)]
// 暂时只支持QA
struct TcpAcceptor {
    addr: SocketAddr,
    socket: Arc<TcpListener>,
    key_store: Keystore,
}

impl TcpAcceptor {
    async fn bind(addr: SocketAddr, key_store: Keystore) -> BuckyResult<TcpAcceptor> {
        match TcpListener::bind(addr.clone()).await {
            Err(e) => {
                warn!("tcp-listener({}) bind failed, err: {}", addr, e);
                Err(BuckyError::from(e))
            },
            Ok(socket) => {
                info!("tcp-listener({}) bind ok.", addr);
                Ok(TcpAcceptor {
                    addr,
                    socket: Arc::new(socket),
                    key_store,
                })
            }
        }
    }

    async fn accept(&self) -> BuckyResult<(PackageBox, MessageSender)> {
        loop {
            match self.socket.accept().await {
                Ok((socket, from_addr)) => {
                    debug!("tcp-listener({}) accept a stream, will read the first package, from {:?}", self.addr, from_addr);
                    match AcceptInterface::accept(socket.clone(), &self.key_store, Duration::from_secs(2)).await {
                        Ok((interface, first_box)) => {
                            break Ok((first_box, MessageSender::Tcp(TcpSender {
                                handle: interface.into()
                            })))
                        },
                        Err(e) => {
                            warn!("tcp-listener({}) accept a stream, but the first package read failed, from {:?}. err: {}", self.addr, from_addr, e);
                            let _ = socket.shutdown(Shutdown::Both);
                        }
                    }
                },
                Err(e) => {
                    match e.kind() {
                        ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::AlreadyExists | ErrorKind::TimedOut => continue,
                        _ => {
                            warn!("tcp-listener({}) accept fatal error({}). will stop.", self.addr, e);
                            break Err(BuckyError::from(e));
                        },
                    }
                }
            }
        }
    }

    fn close(self) {
        // let _ = self.socket.shutdown(Shutdown::Both).await;
    }
}

pub struct UdpSender {
    handle: Arc<UdpInterface>,
    remote_device_id: DeviceId,
    aes_key: AesKey,
    to_addr: SocketAddr,
}

impl UdpSender {
    fn new(handle: Arc<UdpInterface>, remote_device_id: DeviceId, aes_key: AesKey, to_addr: SocketAddr) -> UdpSender {
        UdpSender {
            handle,
            remote_device_id,
            aes_key,
            to_addr
        }
    }
}

impl UdpSender {
    pub fn box_pkg(&self, pkg: DynamicPackage) -> PackageBox {
        let mut package_box = PackageBox::encrypt_box(self.remote_device_id.clone(), self.aes_key.clone());
        package_box.append(vec![pkg]);
        package_box
    }

    pub async fn send(&self,
                  pkg_box: &PackageBox) -> BuckyResult<()> {

        let mut encode_buf = [0; MTU];
        let send_buf = {
            let mut context = PackageBoxEncodeContext::default();

            pkg_box.raw_tail_encode_with_context(&mut encode_buf, &mut context, &None).map_err(|e| {
                error!("udp({}) send-to({}) encode failed, e:{}", self.handle.addr, self.to_addr, &e);
                e
            })?
        };

        match self.handle.socket.send_to(send_buf, self.to_addr).await {
            Ok(len) => {
                assert_eq!(len, send_buf.len());
                return Ok(());
            },
            Err(e) => {
                match e.kind() {
                    ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::AlreadyExists | ErrorKind::TimedOut => {
                        warn!("udp({}) send-to({}) failed({}).", self.handle.addr, self.to_addr, e);
                    },
                    _ => {
                        error!("udp({}) send-to({}) fatal error({}).", self.handle.addr, self.to_addr, e);
                    },
                }
                return Err(BuckyError::from(e));
            }
        }
    }

    pub fn local(&self) -> &SocketAddr {
        &self.handle.addr
    }

    pub fn remote(&self) -> &SocketAddr {
        &self.to_addr
    }

    pub fn session_name(&self) -> String {
        format!("local({})-remote({})",
                self.handle.addr,
                self.to_addr
        )
    }
}

pub struct TcpSender {
    handle: PackageInterface
}

impl TcpSender {
    pub async fn send(&mut self, pkg: DynamicPackage) -> BuckyResult<()> {
        let mut send_buf = [0; MTU];

        match self.handle.send_package(&mut send_buf, pkg, false).await {
            Ok(()) => Ok(()),
            Err(e) => {
                error!("tcp({}) send-to({}) failed error({}).", self.local_str(), self.remote_str(), e);
                return Err(e);
            }
        }
    }

    pub fn session_name(&self) -> String {
        format!("local({})-remote({})",
                self.local_str(),
                self.remote_str()
        )
    }

    pub fn close(self) {
        // 生命周期结束PackageInterface会自动关闭
    }

    fn local_str(&self) -> String {
        self.handle.local().map_or_else(|e| e.msg().to_string(), |a| a.to_string())
    }

    fn remote_str(&self) -> String {
        self.handle.remote().map_or_else(|e| e.msg().to_string(), |a| a.to_string())
    }
}

pub enum MessageSender {
    Udp(UdpSender),
    Tcp(TcpSender),
}

impl MessageSender {
    pub async fn send(&mut self, pkg: DynamicPackage) -> BuckyResult<()> {
        match self {
            MessageSender::Udp(u) => {
                let pkg_box = u.box_pkg(pkg);
                u.send(&pkg_box).await
            },
            MessageSender::Tcp(t) => t.send(pkg).await
        }
    }

    pub fn local(&self) -> Option<SocketAddr> {
        match self {
            MessageSender::Udp(u) => Some(u.handle.addr),
            MessageSender::Tcp(t) => t.handle.local().map_or(None, |a| Some(a)),
        }
    }

    pub fn remote(&self) -> Option<SocketAddr> {
        match self {
            MessageSender::Udp(u) => Some(u.to_addr),
            MessageSender::Tcp(t) => t.handle.remote().map_or(None, |a| Some(a)),
        }
    }

    pub fn session_name(&self) -> String {
        match self {
            MessageSender::Udp(u) => u.session_name(),
            MessageSender::Tcp(t) => t.session_name()
        }
    }
}

struct RecvPending {}

impl RecvPending {
    fn new() -> RecvPending {
        RecvPending {}
    }
}

impl Future for RecvPending {
    type Output = BuckyResult<(PackageBox, MessageSender)>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}