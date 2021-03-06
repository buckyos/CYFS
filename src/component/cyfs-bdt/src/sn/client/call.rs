use cyfs_base::*;
use std::collections::HashMap;
use std::sync::{Arc, atomic, atomic::{AtomicU64, AtomicU32}, RwLock};
use std::future::Future;
use crate::{TempSeqGenerator, TempSeq};
use std::time::{Instant, Duration};
use crate::interface::{udp::{Interface, PackageBoxEncodeContext}, tcp};
use std::pin::Pin;
use async_std::task;
use crate::protocol::{Exchange, SnCall, SnCallResp, PackageBox, PackageCmdCode, DynamicPackage};
use crate::sn::client::{Config};
use std::task::Waker;
use futures::task::{Context, Poll};
use crate::stack::{WeakStack, Stack};

pub struct CallManager {
    stack: WeakStack,
    seq_genarator: TempSeqGenerator,
    timeout: Duration,
    call_interval: Duration,
    call_sessions: Arc<RwLock<HashMap<TempSeq, Arc<CallSession>>>>,
    on_stop: Arc<dyn Fn(TempSeq) + Send + Sync>,
}

impl CallManager {
    pub fn create(stack: WeakStack, config: &Config) -> CallManager {
        let call_sesssions = Arc::new(RwLock::new(Default::default()));
        CallManager {
            stack,
            seq_genarator: TempSeqGenerator::new(),
            timeout: config.call_timeout,
            call_interval: config.call_interval,
            call_sessions: call_sesssions.clone(),
            on_stop: Arc::new(move |seq: TempSeq| {call_sesssions.write().unwrap().remove(&seq);})
        }
    }

    pub fn call(&self,
                  reverse_endpoints: &[Endpoint],
                  remote_peerid: &DeviceId,
                  sn: &Device,
                  is_always_call: bool,
                  is_encrypto: bool,
                  with_local: bool,
                  payload_generater: impl Fn(&SnCall) -> Vec<u8>
    ) -> impl Future<Output = Result<Device, BuckyError>> {
        let seq = self.seq_genarator.generate();
        let call_result = Arc::new(RwLock::new(CallResult { found_peer: None, waker: None }));

        let session = Arc::new(CallSession::create(self,
                                                   reverse_endpoints,
                                                   remote_peerid,
                                                   sn,
                                                   is_always_call,
                                                   is_encrypto,
                                                   with_local,
                                                   payload_generater,
                                                   seq,
                                                   call_result.clone()));

        {
            let mut sessions = self.call_sessions.write().unwrap();
            sessions.insert(seq, session.clone());
        }

        session.start(self.call_interval, self.timeout, self.on_stop.clone());

        CallFuture {
            call_result
        }
    }

    pub fn on_call_resp(&self, resp: &SnCallResp, from: &Endpoint) -> Result<(), BuckyError> {
        let session = {
            let sessions = self.call_sessions.read().unwrap();
            match sessions.get(&resp.seq) {
                Some(s) => Some((*s).clone()),
                None => None
            }
        };

        match session {
            Some(s) => s.on_call_resp(resp, from),
            None => {
                log::warn!("call-resp, seq: {:?}, maybe has complete", resp.seq);
                Err(BuckyError::new(BuckyErrorCode::NotFound, "not found the call"))
            }
        }
    }
}

struct CallResult {
    found_peer: Option<Result<Device, BuckyError>>,
    waker: Option<Waker>,
}

struct CallFuture {
    call_result: Arc<RwLock<CallResult>>
}

impl Future for CallFuture {
    type Output = Result<Device, BuckyError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut result = self.call_result.write().unwrap();
        match result.found_peer.as_mut() {
            Some(r) => {
                let mut ret = Err(BuckyError::new(BuckyErrorCode::Timeout, "has finish"));
                std::mem::swap(r, &mut ret);
                Poll::Ready(ret)
            },
            None => {
                result.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct CallSession {
    start_time: Instant,
    seq: TempSeq,
    clients: Arc<HashMap<DeviceId, CallClient>>,
    resp_client_count: AtomicU32,
    call_result: Arc<RwLock<CallResult>>
}

impl CallSession {
    fn create(mgr: &CallManager,
              reverse_endpoints: &[Endpoint],
              remote_peerid: &DeviceId,
              sn: &Device,
              is_always_call: bool,
              is_encrypto: bool,
              with_local: bool,
              payload_generater: impl Fn(&SnCall) -> Vec<u8>,
              seq: TempSeq,
              call_result: Arc<RwLock<CallResult>>
    ) -> CallSession {
        let sn_peerid = sn.desc().device_id();

        let stack = Stack::from(&mgr.stack);
        let mut call_pkg = {
            let local_peer = stack.device_cache().local().clone();
            CallSession::init_call_pkg(
                &stack.local_device_id(), 
                local_peer, 
                reverse_endpoints, 
                stack.proxy_manager().active_proxies(),  
                sn_peerid.clone(), 
                remote_peerid, 
                is_always_call, 
                seq)
        };
        call_pkg.payload = SizedOwnedData::from(payload_generater(&call_pkg));

        if !with_local && stack.sn_client().ping.is_cached(&sn_peerid) {
            call_pkg.peer_info = None;
        }

        log::debug!("call begin, to: {}, seq: {}, with payload(len={}).", remote_peerid, seq.value(), call_pkg.payload.len());

        let client = CallClient::create(mgr, &sn_peerid, sn, is_encrypto, call_pkg);

        let mut clients: HashMap<DeviceId, CallClient> = Default::default();
        clients.insert(sn_peerid, client);

        CallSession {
            seq,
            start_time: Instant::now(),
            clients: Arc::new(clients),
            resp_client_count: AtomicU32::new(0),
            call_result
        }
    }

    fn init_call_pkg(local_peerid: &DeviceId,
                     local_peer: Device,
                     reverse_endpoints: &[Endpoint], 
                     active_pn_list: Vec<DeviceId>, 
                     sn_peerid: DeviceId,
                     remote_peerid: &DeviceId,
                     is_always_call: bool,
                     seq: TempSeq) -> SnCall {
        SnCall {
            seq: seq,
            to_peer_id: remote_peerid.clone(),
            from_peer_id: local_peerid.clone(),
            sn_peer_id: sn_peerid,
            reverse_endpoint_array: if reverse_endpoints.len() == 0 { None } else { Some(Vec::from(reverse_endpoints.clone())) },
            active_pn_list: if active_pn_list.len() > 0 {
                Some(active_pn_list)
            } else {
                None
            }, 
            peer_info: Some(local_peer),
            payload: SizedOwnedData::from(vec![]),
            send_time: 0,
            is_always_call
        }
    }

    fn start(&self, resend_interval: Duration, timeout: Duration, on_stop: Arc<dyn Fn(TempSeq) + Send + Sync>) {
        let clients = self.clients.clone();
        let call_result = self.call_result.clone();
        let start_time = self.start_time.clone();
        let seq = self.seq;
        // <TODO>??????????????????????????????????????????
        // <TODO>?????????????????????????????????????????????

        task::spawn(async move {
            let mut is_tcp_try = false;
            let mut sign_futures = vec![];
            for client in clients.values() {
                unsafe {
                    let client = &mut *(Arc::as_ptr(&client.inner) as *mut CallClientInner);
                    sign_futures.push(client.sign_exchange());
                }
            }
            futures::future::join_all(sign_futures).await;

            loop {
                // UDP??????
                let mut send_count = 0;
                for client in clients.values() {
                    send_count += client.send_udp_pkg();
                }

                // UDP???????????????????????????????????????TCP????????????
                if send_count > 0 || is_tcp_try {
                    task::sleep(resend_interval).await;
                }

                let waker = {
                    {
                        let result = call_result.read().unwrap();
                        if let Some(_) = result.found_peer {
                            break;
                        }
                    }

                    if start_time.elapsed() >= timeout {
                        let id = clients.values().next().unwrap().session_id();
                        log::warn!("call-finish, to: {}, seq: {}, find peer timeout, no sn responce.", id.0.to_string(), id.1.value());
                        let mut result = call_result.write().unwrap();
                        result.found_peer = Some(Err(BuckyError::new(BuckyErrorCode::Timeout, "no sn responce")));
                        Some(result.waker.clone())
                    } else {
                        None
                    }
                };

                if let Some(w) = waker {
                    if let Some(w) = w {
                        w.wake();
                    }
                    on_stop(seq);
                    break;
                }

                if !is_tcp_try {
                    is_tcp_try = true;
                    for client in clients.values() {
                        client.try_send_tcp_pkg(timeout);
                    }
                }
            }
        });
    }

    fn on_call_resp(&self, resp: &SnCallResp, from: &Endpoint) -> Result<(), BuckyError> {
        let client = self.clients.get(&resp.sn_peer_id);
        if let Some(c) = client {
            let done_waker = {
                let has_found = self.call_result.read().unwrap().found_peer.is_some();
                let id = self.session_id();
                if has_found {
                    // ??????????????????????????????
                    log::info!("call-resp, to: {}, seq: {}, sn: {} has finished before.", id.0.to_string(), id.1.value(), resp.sn_peer_id.to_string());
                    return Ok(());
                } else {
                    let mut call_result = self.call_result.write().unwrap();
                    match resp.to_peer_info.as_ref() {
                        Some(desc) => {
                            log::info!("call-resp, to: {}, seq: {}, sn: {}, eps: {} found target device.",
                             id.0.to_string(),
                              id.1.value(),
                               resp.sn_peer_id.to_string(),
                                desc.connect_info().endpoints().iter().map(|ep| ep.to_string()).collect::<Vec<String>>().concat());
                            call_result.found_peer = Some(Ok(desc.clone()));
                            Some(call_result.waker.clone())
                        }
                        None => {
                            log::info!("call-resp, to: {}, seq: {}, sn: {} not found target device.", id.0.to_string(), id.1.value(), resp.sn_peer_id.to_string());
                            let mut not_found = || -> Option<Option<Waker>> {
                                let now = self.start_time.elapsed().as_millis() as u64;
                                let mut last_resp_time = 0;
                                c.on_call_resp(resp, from, now, &mut last_resp_time);

                                // ?????????????????????????????????sn??????????????????wake?????????????????????
                                if last_resp_time == 0 {
                                    let last_count = self.resp_client_count.fetch_add(1, atomic::Ordering::SeqCst) as usize;
                                    if last_count == self.clients.len() - 1 {
                                        log::info!("call-resp, to: {}, seq: {}, sn: {} all sn responced, but no target device found.", id.0.to_string(), id.1.value(), resp.sn_peer_id.to_string());
                                        call_result.found_peer = Some(Err(BuckyError::new(BuckyErrorCode::NotFound, "not found the peer")));
                                        return Some(call_result.waker.clone());
                                    }
                                }
                                None
                            };
                            not_found()
                        }
                    }
                }
            };

            // ??????????????????/??????SN???????????????wake??????
            if let Some(w) = done_waker {
                if let Some(w) = w {
                    w.wake();
                }

                // <TODO>?????????

                self.stop();
            }

            Ok(())
        } else {
            unreachable!()
        }
    }

    fn stop(&self) {}

    fn session_id(&self) -> (&DeviceId, TempSeq) {
        let any_client = self.clients.values().next().unwrap();
        any_client.session_id()
    }
}

enum SendPackage {
    Exchange(Exchange),
    Call(SnCall)
}

struct CallClientInner {
    stack: WeakStack,
    sn_peerid: DeviceId,
    sn: Device,
    aes_key: Option<(AesKey, bool)>, // <key,exchange>
    pkgs: Vec<SendPackage>,
    last_resp_time: AtomicU64,
}

impl CallClientInner {
    fn init_pkgs(&mut self, mut call_pkg: SnCall) {
        match self.aes_key.as_ref() {
            Some((_, is_exchange)) if *is_exchange => {
                let stack = Stack::from(&self.stack);
                let exchg = Exchange {
                    sequence: call_pkg.seq,
                    seq_key_sign: Signature::default(),
                    from_device_id: stack.local_device_id().clone(),
                    send_time: 0,
                    from_device_desc: match call_pkg.peer_info.as_ref() {
                        Some(from) => from.clone(),
                        None => stack.device_cache().local()
                    },
                };
                self.pkgs.push(SendPackage::Exchange(exchg));
            }
            _ => {}
        };

        call_pkg.sn_peer_id = self.sn_peerid.clone();
        self.pkgs.push(SendPackage::Call(call_pkg));
    }

    async fn sign_exchange(&mut self) {
        if let SendPackage::Exchange(exchg) = self.pkgs.get_mut(0).unwrap() {
            let _ = exchg.sign(&self.aes_key.as_ref().unwrap().0, Stack::from(&self.stack).keystore().signer()).await;
        }
    }
}

#[derive(Clone)]
struct CallClient {
    inner: Arc<CallClientInner>
}

impl CallClient {
    fn create(mgr: &CallManager, sn_peerid: &DeviceId, sn: &Device, is_encrypto: bool, call_pkg: SnCall) -> CallClient {
        let key = if is_encrypto {
            let key = Stack::from(&mgr.stack).keystore().create_key(&sn_peerid, false);
            Some((key.aes_key.clone(), !key.is_confirmed))
        } else {
            None
        };

        let mut inner = CallClientInner {
            stack: mgr.stack.clone(),
            sn_peerid: sn_peerid.clone(),
            sn: sn.clone(),
            aes_key: key,
            pkgs: vec![],
            last_resp_time: AtomicU64::new(0)
        };
        inner.init_pkgs(call_pkg);

        let client = CallClient {
            inner: Arc::new(inner)
        };

        client
    }

    fn on_call_resp(&self, _resp: &SnCallResp, _from: &Endpoint, now: u64, last_resp_time: &mut u64) {
        *last_resp_time = self.inner.last_resp_time.swap(now, atomic::Ordering::Release);
    }

    fn prepare_pkgs_to_send(&self) -> Result<PackageBox, BuckyError> {
        assert!(self.inner.aes_key.is_some());
        // <TODO>?????????????????????
        let mut pkg_box = PackageBox::encrypt_box(self.inner.sn_peerid.clone(), self.inner.aes_key.as_ref().unwrap().0.clone());
        let now_abs = bucky_time_now();
        for pkg in self.inner.pkgs.as_slice() {
            match pkg {
                SendPackage::Exchange(exchg) => {
                    let mut exchg = (*exchg).clone();
                    exchg.send_time = now_abs;
                    pkg_box.push(exchg);
                },
                SendPackage::Call(call) => {
                    let mut call = (*call).clone();
                    call.send_time = now_abs;
                    pkg_box.push(call);
                }
            }
        }

        Ok(pkg_box)
    }

    fn send_udp_pkg(&self) -> usize {
        // ???????????????
        if self.inner.last_resp_time.load(atomic::Ordering::Acquire) > 0 {
            return 0;
        }

        let stack = Stack::from(&self.inner.stack);
        if stack.net_manager().listener().udp().len() == 0 {
            return 0;
        }

        if let Ok(pkg_box) = self.prepare_pkgs_to_send() {
            let mut context = PackageBoxEncodeContext::from(self.inner.sn.desc());

            struct SendIter<'a> {
                from: &'a Vec<Interface>,
                to: Vec<&'a Endpoint>,
                sub_pos: usize,
                pos: usize,
            }

            impl <'a> Iterator for SendIter<'a> {
                type Item = (Interface, Endpoint);

                fn next(&mut self) -> Option<Self::Item> {
                    let from = self.from.get(self.pos);
                    if let Some(from) = from {
                        let ep = self.to.get(self.sub_pos);
                        if let Some(ep) = ep {
                            self.sub_pos += 1;
                            Some(((*from).clone(), (*ep).clone()))
                        } else {
                            self.pos += 1;
                            self.sub_pos = 0;
                            self.next()
                        }
                    } else {
                        None
                    }
                }
            }

            let stack = Stack::from(&self.inner.stack);
            let listener = stack.net_manager().listener();
            let send_iter = SendIter {
                from: listener.udp(),
                to: self.inner.sn.connect_info().endpoints().iter().filter(|ep| ep.is_udp()).collect(),
                sub_pos: 0,
                pos: 0
            };

            let r = Interface::send_box_mult(&mut context, &pkg_box, send_iter, |from, to, result| {
                let id = self.session_id();
                log::debug!("call to: {}, seq: {}, from {} to {}, result: {:?}", id.0.to_string(), id.1.value(), from.local(), to, result);
                true
            });

            match r {
                Ok(send_count) => send_count,
                Err(e) => {
                    let id = self.session_id();
                    log::debug!("call to: {}, seq: {}, failed, err: {:?}", id.0.to_string(), id.1.value(), e);
                    0
                }
            }
        } else {
            0
        }
    }

    fn try_send_tcp_pkg(&self, time_limit: Duration) {
        let inner = self.inner.clone();
        let pkg_box = match self.prepare_pkgs_to_send() {
            Ok(pkg_box) => pkg_box,
            Err(e) => {
                log::error!("call prepare pkg for tcp failed, e: {}", e);
                return;
            }
        };
        
        task::spawn(async move {
            let remote_eps = inner.sn.connect_info().endpoints();
            let mut connect_futures = vec![];
            for ep in remote_eps {
                if ep.is_tcp() {
                    connect_futures.push(
                        Box::pin(tcp::Interface::connect(ep.clone(), inner.sn_peerid.clone(), inner.sn.desc().clone(), pkg_box.key().clone(), time_limit))
                    );
                }
            }

            let mut connect_futures_container = Some(connect_futures);
            let connect_result = loop {
                let connect_futures = connect_futures_container.take().unwrap();
                if connect_futures.is_empty() {
                    break Err(BuckyError::new(BuckyErrorCode::Failed, "all failed"));
                }

                let (result, _, remain) = futures::future::select_all(connect_futures).await;
                match result {
                    Ok(tcp) => break Ok(tcp),
                    Err(_) => connect_futures_container = Some(remain)
                }
            };

            if let Ok(tcp_interface) = connect_result {
                let stack = Stack::from(&inner.stack);
                match tcp_interface.confirm_connect(&stack, pkg_box.into(), time_limit).await {
                    Ok(resp) => {
                        let resp: Vec<DynamicPackage> = resp.into();
                        for pkg in resp {
                            if let PackageCmdCode::SnCallResp = pkg.cmd_code() {
                                if let Some(resp) = pkg.as_any().downcast_ref::<SnCallResp>() {
                                    let _ = stack.sn_client().call.on_call_resp(resp, tcp_interface.remote_endpoint());
                                }
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("tcp call resp failed, err: {:?}", e);
                    }
                }
            }
        });
    }

    fn session_id(&self) -> (&DeviceId, TempSeq) {
        for pkg in self.inner.pkgs.as_slice() {
            if let SendPackage::Call(call) = pkg {
                return (&call.to_peer_id, call.seq);
            }
        }
        unreachable!()
    }
}
