use async_std::task;
use futures::executor::ThreadPool;
use log::*;
use std::{
    any::Any,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    time::Duration,
};

use cyfs_base::*;

use crate::{
    history::keystore::{self, Keystore},
    protocol::{*, v0::*},
    types::*,
};

use super::{
    call_stub::CallStub,
    net_listener::{MessageSender, NetListener, UdpSender},
    peer_manager::PeerManager,
    receipt::*,
    resend_queue::ResendQueue,
};

// const TRACKER_INTERVAL: Duration = Duration::from_secs(60);
// struct CallTracker {
//     calls: HashMap<TempSeq, (u64, Instant, DeviceId)>, // <called_seq, (call_send_time, called_send_time)>
//     begin_time: Instant,
// }

struct ServiceImpl {
    seq_generator: TempSeqGenerator,
    key_store: Keystore,
    local_device_id: DeviceId,
    local_device: Device,
    stopped: AtomicBool,
    contract: Box<dyn SnServiceContractServer + Send + Sync>,
    thread_pool: ThreadPool,

    // call_tracker: CallTracker,
    peer_mgr: PeerManager,
    resend_queue: ResendQueue,
    call_stub: CallStub,
}

#[derive(Clone)]
pub struct SnService(Arc<ServiceImpl>);

impl SnService {
    pub fn new(
        local_device: Device,
        local_secret: PrivateKey,
        contract: Box<dyn SnServiceContractServer + Send + Sync>,
    ) -> SnService {
        let thread_pool = ThreadPool::new().unwrap();

        Self(Arc::new(ServiceImpl {
            seq_generator: TempSeqGenerator::new(),
            key_store: Keystore::new(
                local_secret.clone(),
                local_device.desc().clone(),
                RsaCPUObjectSigner::new(
                    local_device.desc().public_key().clone(),
                    local_secret.clone(),
                ),
                keystore::Config {
                    // <TODO>提供配置
                    active_time: Duration::from_secs(600),
                    capacity: 100000,
                },
            ),
            resend_queue: ResendQueue::new(thread_pool.clone(), Duration::from_millis(200), 5),
            local_device_id: local_device.desc().device_id(),
            local_device: local_device.clone(),
            stopped: AtomicBool::new(false),
            peer_mgr: PeerManager::new(Duration::from_secs(300)),
            call_stub: CallStub::new(),
            thread_pool,
            contract,
            // call_tracker: CallTracker {
            //     calls: Default::default(),
            //     begin_time: Instant::now()
            // }
        }))
    }

    pub async fn start(&self) -> BuckyResult<()> {
        let mut endpoints_v4 = vec![];
        let mut endpoints_v6 = vec![];
        for endpoint in self.0.local_device.connect_info().endpoints() {
            let mut addr = endpoint.addr().clone();
            if addr.is_ipv4() {
                addr.set_ip("0.0.0.0".parse().unwrap());
                endpoints_v4.push(Endpoint::from((endpoint.protocol(), addr)));
            } else {
                addr.set_ip("::".parse().unwrap());
                endpoints_v6.push(Endpoint::from((endpoint.protocol(), addr)));
            };
        }

        let _listener = match NetListener::listen(&endpoints_v6, &endpoints_v4, self.clone()).await
        {
            Ok((listener, udp_count, _)) => {
                if udp_count == 0 {
                    log::error!("sn-minner start failed for all udp-endpoints listen failed.");
                    Err(BuckyError::new(
                        BuckyErrorCode::Failed,
                        "all udp-endpoint listen failed",
                    ))
                } else {
                    Ok(listener)
                }
            }
            Err(e) => Err(e),
        }?;

        // 清理过期数据
        let timer = {
            let service = self.clone();
            task::spawn(async move {
                loop {
                    {
                        if service.is_stopped() {
                            return;
                        }
                        service.clean_timeout_resource();
                    }
                    task::sleep(Duration::from_micros(100000)).await;
                }
            })
        };

        // 没有stop
        timer.await;

        Ok(())
    }

    pub fn stop(&self) {
        self.0.stopped.store(true, atomic::Ordering::Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.0.stopped.load(atomic::Ordering::Relaxed)
    }

    pub fn local_device_id(&self) -> &DeviceId {
        &self.0.local_device_id
    }

    pub(super) fn key_store(&self) -> &Keystore {
        &self.0.key_store
    }

    fn resend_queue(&self) -> &ResendQueue {
        &self.0.resend_queue
    }

    fn peer_manager(&self) -> &PeerManager {
        &self.0.peer_mgr
    }

    pub(super) fn thread_pool(&self) -> &ThreadPool {
        &self.0.thread_pool
    }

    fn send_resp(&self, mut sender: MessageSender, pkg: DynamicPackage, send_log: String) {
        self.thread_pool().spawn_ok(async move {
            if let Err(e) = sender.send(pkg).await {
                warn!("{} send failed. error: {}.", send_log, e.to_string());
            } else {
                debug!("{} send ok.", send_log);
            }

            if let MessageSender::Tcp(tcp_sender) = sender {
                tcp_sender.close()
            }
        });
    }

    fn send_resp_udp(&self, sender: Arc<UdpSender>, pkg: DynamicPackage, send_log: String) {
        self.thread_pool().spawn_ok(async move {
            let pkg_box = sender.box_pkg(pkg);
            if let Err(e) = sender.send(&pkg_box).await {
                warn!("{} send failed. error: {}.", send_log, e.to_string());
            } else {
                debug!("{} send ok.", send_log);
            }
        });
    }

    fn clean_timeout_resource(&self) {
        let now = bucky_time_now();
        self.peer_manager().try_knock_timeout(now);
        self.resend_queue().try_resend(now);
        self.0.call_stub.recycle(now);
        // {
        //     let tracker = &mut self.call_tracker;
        //     if let Ordering::Greater = now.duration_since(tracker.begin_time).cmp(&TRACKER_INTERVAL) {
        //         tracker.calls.clear();
        //         tracker.begin_time = now;
        //     }
        // }
    }

    pub(super) fn handle(&self, mut pkg_box: PackageBox, resp_sender: MessageSender) {
        let first_pkg = pkg_box.pop();
        if first_pkg.is_none() {
            warn!("fetch none pkg");
            return;
        }

        let send_time = bucky_time_now();
        let first_pkg = first_pkg.unwrap();
        let cmd_pkg = match first_pkg.cmd_code() {
            PackageCmdCode::Exchange => {
                let exchg = <Box<dyn Any + Send>>::downcast::<Exchange>(first_pkg.into_any()); // pkg.into_any().downcast::<Exchange>();
                if let Ok(exchg) = exchg {
                    self.key_store()
                        .add_key(pkg_box.enc_key(), pkg_box.remote(), exchg.mix_key());
                } else {
                    warn!("fetch exchange failed, from: {:?}.", resp_sender.remote());
                    return;
                }

                match pkg_box.pop() {
                    Some(pkg) => pkg,
                    None => {
                        warn!("fetch none cmd-pkg, from: {:?}.", resp_sender.remote());
                        return;
                    }
                }
            }
            _ => first_pkg,
        };

        match cmd_pkg.cmd_code() {
            PackageCmdCode::SnPing => {
                let ping_req = <Box<dyn Any + Send>>::downcast::<SnPing>(cmd_pkg.into_any());
                if let Ok(ping_req) = ping_req {
                    self.handle_ping(
                        ping_req,
                        resp_sender,
                        Some((pkg_box.enc_key(), pkg_box.remote())),
                        send_time,
                    );
                } else {
                    warn!("fetch ping-req failed, from: {:?}.", resp_sender.remote());
                    return;
                }
            }
            PackageCmdCode::SnCall => {
                let call_req = <Box<dyn Any + Send>>::downcast::<SnCall>(cmd_pkg.into_any());
                if let Ok(call_req) = call_req {
                    self.handle_call(
                        call_req,
                        resp_sender,
                        Some((pkg_box.enc_key(), pkg_box.remote())),
                        send_time,
                    );
                } else {
                    warn!("fetch sn-call failed, from: {:?}.", resp_sender.remote());
                    return;
                }
            }
            PackageCmdCode::SnCalledResp => {
                let called_resp =
                    <Box<dyn Any + Send>>::downcast::<SnCalledResp>(cmd_pkg.into_any());
                if let Ok(called_resp) = called_resp {
                    self.handle_called_resp(called_resp, Some(pkg_box.enc_key()))
                } else {
                    warn!(
                        "fetch sn-called-resp failed, from: {:?}.",
                        resp_sender.remote()
                    );
                    return;
                }
            }
            _ => warn!("invalid cmd-package, from: {:?}.", resp_sender.remote()),
        }
    }

    fn handle_ping(
        &self,
        ping_req: Box<SnPing>,
        resp_sender: MessageSender,
        encryptor: Option<(&AesKey, &DeviceId)>,
        send_time: Timestamp,
    ) {
        let from_peer_id = match ping_req.from_peer_id.as_ref() {
            Some(id) => id,
            None => match encryptor {
                Some((_, id)) => id,
                None => {
                    warn!(
                        "[ping from 'unknow-deviceid' seq({})] without from peer-desc.",
                        ping_req.seq.value()
                    );
                    return;
                }
            },
        };

        let aes_key = encryptor.map(|(key, _)| key);

        let log_key = format!(
            "[ping from {} seq({})]",
            from_peer_id.to_string(),
            ping_req.seq.value()
        );
        let resp_sender = match resp_sender {
            MessageSender::Tcp(_) => {
                warn!("{} from tcp.", log_key);
                return;
            }
            MessageSender::Udp(u) => Arc::new(u),
        };

        info!("{}", log_key);

        // let (result, endpoints, receipt) = if let Some((accept, local_receipt)) = self.ping_receipt(&ping_req, from_peer_id) {
        //     let receipt = match accept {
        //         IsAcceptClient::Refuse => {
        //             return;
        //         }
        //         IsAcceptClient::Accept(is_request_receipt) => if is_request_receipt {
        //             Some(local_receipt)
        //         } else {
        //             None
        //         }
        //     };

        //     info!("{} from-endpoint: {}", log_key, resp_sender.remote());
        //     (BuckyErrorCode::Ok as u8, vec![Endpoint::from((Protocol::Udp, resp_sender.remote().clone()))], receipt)
        // } else {
        //     (BuckyErrorCode::NotFound as u8, vec![], None)
        // };

        if !self.peer_manager().peer_heartbeat(
            from_peer_id.clone(),
            &ping_req.peer_info,
            resp_sender.clone(),
            aes_key,
            send_time,
            ping_req.seq,
        ) {
            warn!("{} cache peer failed. the ping maybe is timeout.", log_key);
            return;
        };

        let ping_resp = SnPingResp {
            seq: ping_req.seq,
            sn_peer_id: self.local_device_id().clone(),
            result: BuckyErrorCode::Ok.into_u8(),
            peer_info: Some(self.0.local_device.clone()),
            end_point_array: vec![Endpoint::from((
                Protocol::Udp,
                resp_sender.remote().clone(),
            ))],
            receipt: None,
        };

        self.send_resp_udp(
            resp_sender,
            DynamicPackage::from(ping_resp),
            format!("{}", log_key),
        );
    }

    // fn verify_receipt_sign(
    //     &self,
    //     client_desc: &DeviceDesc,
    //     signed_receipt: &Option<ReceiptWithSignature>) -> bool {
    //     match signed_receipt {
    //         None => false,
    //         Some(receipt) => {
    //             receipt.receipt().verify(sn_peerid, receipt.signature(), client_desc)
    //         }
    //     }
    // }

    // // 处理ping服务证明
    // fn ping_receipt(&self, ping_req: &SnPing, from_id: &DeviceId) -> Option<(IsAcceptClient, SnServiceReceipt)> {
    //     let mut cache_peer = self.peer_mgr.find_peer(from_id, FindPeerReason::Other);

    //     let (device, local_receipt, last_receipt_request_time) = match &cache_peer {
    //         Some(cache) => (&cache.desc, cache.receipt.clone(), cache.last_receipt_request_time),
    //         None => {
    //             let dev = match ping_req.peer_info.as_ref() {
    //                 Some(dev) => dev,
    //                 None => return None,
    //             };
    //             (
    //                 dev,
    //                 SnServiceReceipt::default(),
    //                 ReceiptRequestTime::None
    //             )
    //         }
    //     };

    //     let is_verify_ok = self.verify_receipt_sign(ping_req.peer_info.desc(), &ping_req.receipt);
    //     let client_receipt = if is_verify_ok { &ping_req.receipt } else { &None };
    //     let check_receipt = self.contract.check_receipt(device, &local_receipt, client_receipt, &last_receipt_request_time);

    //     let is_reset_receipt = if is_verify_ok {
    //         match cache_peer.as_mut() {
    //             Some(cache_peer) => match last_receipt_request_time {
    //                 ReceiptRequestTime::Wait(t) => {
    //                     cache_peer.last_receipt_request_time = ReceiptRequestTime::Last(t);
    //                     // 重置统计计数
    //                     true
    //                 }
    //                 _ => false
    //             }
    //             None => false
    //         }
    //     } else {
    //         false
    //     };

    //     let is_request_receipt = match check_receipt {
    //         IsAcceptClient::Refuse => {
    //             warn!("[ping from {} seq({})] refused by contract.", from_id, ping_req.seq.value());
    //             return Some((IsAcceptClient::Refuse, local_receipt))
    //         },
    //         IsAcceptClient::Accept(r) => r,
    //     };

    //     if let Some(cache_peer) = cache_peer {
    //         if is_reset_receipt {
    //             cache_peer.receipt.start_time = SystemTime::now();
    //             cache_peer.receipt.ping_count = 0;
    //             cache_peer.receipt.ping_resp_count = 0;
    //             cache_peer.receipt.called_count = 0;
    //             cache_peer.receipt.call_peer_count = 0;
    //             cache_peer.call_peers.clear();
    //         }

    //         if cache_peer.last_ping_seq != ping_req.seq {
    //             cache_peer.receipt.ping_count += 1;
    //             cache_peer.receipt.ping_resp_count += 1;
    //             cache_peer.last_ping_seq = ping_req.seq;
    //         }

    //         if is_request_receipt {
    //             if let ReceiptRequestTime::Last(_) = cache_peer.last_receipt_request_time { // 一次新的请求
    //                 cache_peer.last_receipt_request_time = ReceiptRequestTime::Wait(SystemTime::now());
    //             }
    //         }
    //     }

    //     Some((IsAcceptClient::Accept(is_request_receipt), local_receipt))
    // }

    fn handle_call(
        &self,
        mut call_req: Box<SnCall>,
        resp_sender: MessageSender,
        _encryptor: Option<(&AesKey, &DeviceId)>,
        _send_time: Timestamp,
    ) {
        let from_peer_id = &call_req.from_peer_id;
        let log_key = format!(
            "[call {}->{} seq({})]",
            from_peer_id.to_string(),
            call_req.to_peer_id.to_string(),
            call_req.seq.value()
        );
        info!("{}.", log_key);
        // if let IsAcceptClient::Refuse = self.contract.verify_auth(&call_req.to_peer_id) {
        //     warn!("{} refused by contract.", log_key);
        //     send_responce(self,
        //                   resp_sender,
        //                   call_req.seq,
        //                   BuckyErrorCode::PermissionDenied,
        //                   None,
        //                   log_key.as_str()
        //     );
        //     return;
        // }

        // if let Some(cached_from) = self.peer_mgr.find_peer(from_peer_id, FindPeerReason::CallFrom(*send_time)) {
        //     if &cached_from.last_call_time > send_time {
        //         warn!("{} ignore for timeout.", log_key);
        //         return;
        //     } else {
        //         if from_peer_desc.is_none() {
        //             from_peer_desc = Some(cached_from.desc.clone());
        //         }
        //     }
        // } else {
        //     warn!("{} without from-desc.", log_key);
        //     call_result = BuckyErrorCode::NotFound;
        // };

        let call_resp =
            if let Some(to_peer_cache) = self.peer_manager().find_peer(&call_req.to_peer_id) {
                // Self::call_stat_contract(to_peer_cache, &call_req);
                let from_peer_desc = if call_req.peer_info.is_none() {
                    self.peer_manager().find_peer(from_peer_id).map(|c| c.desc)
                } else {
                    call_req.peer_info
                };

                if let Some(from_peer_desc) = from_peer_desc {
                    info!(
                        "{} to-peer found, endpoints: {}, always_call: {}, to-peer.is_wan: {}.",
                        log_key,
                        endpoints_to_string(to_peer_cache.desc.connect_info().endpoints()),
                        call_req.is_always_call,
                        to_peer_cache.is_wan
                    );

                    if self.0.call_stub.insert(from_peer_id, &call_req.seq) {
                        if call_req.is_always_call || !to_peer_cache.is_wan {
                            let called_seq = self.0.seq_generator.generate();
                            let mut called_req = SnCalled {
                                seq: called_seq,
                                to_peer_id: call_req.to_peer_id.clone(),
                                sn_peer_id: self.local_device_id().clone(),
                                peer_info: from_peer_desc,
                                call_seq: call_req.seq,
                                call_send_time: call_req.send_time,
                                payload: SizedOwnedData::from(vec![]),
                                reverse_endpoint_array: vec![],
                                active_pn_list: vec![],
                            };

                            std::mem::swap(&mut call_req.payload, &mut called_req.payload);
                            if let Some(eps) = call_req.reverse_endpoint_array.as_mut() {
                                std::mem::swap(eps, &mut called_req.reverse_endpoint_array);
                            }
                            if let Some(pn_list) = call_req.active_pn_list.as_mut() {
                                std::mem::swap(pn_list, &mut called_req.active_pn_list);
                            }

                            let called_log =
                                format!("{} called-req seq({})", log_key, called_seq.value());
                            log::debug!(
                                "{} will send with payload(len={}) pn_list({:?}).",
                                called_log,
                                called_req.payload.len(),
                                called_req.active_pn_list
                            );
                            self.resend_queue().send(
                                to_peer_cache.sender.clone(),
                                DynamicPackage::from(called_req),
                                called_seq.value(),
                                called_log,
                            );
                            // self.call_tracker.calls.insert(called_seq, (call_req.send_time, Instant::now(), call_req.to_peer_id.clone()));
                        }
                    } else {
                        info!("{} ignore send called req for already exists.", log_key);
                    }

                    SnCallResp {
                        seq: call_req.seq,
                        sn_peer_id: self.local_device_id().clone(),
                        result: BuckyErrorCode::Ok.into_u8(),
                        to_peer_info: Some(to_peer_cache.desc),
                    }
                } else {
                    warn!("{} without from-desc.", log_key);

                    SnCallResp {
                        seq: call_req.seq,
                        sn_peer_id: self.local_device_id().clone(),
                        result: BuckyErrorCode::NotFound.into_u8(),
                        to_peer_info: None,
                    }
                }
            } else {
                warn!("{} to-peer not found.", log_key);
                SnCallResp {
                    seq: call_req.seq,
                    sn_peer_id: self.local_device_id().clone(),
                    result: BuckyErrorCode::NotFound.into_u8(),
                    to_peer_info: None,
                }
            };

        self.send_resp(
            resp_sender,
            DynamicPackage::from(call_resp),
            format!("{} call-resp", log_key),
        );
    }

    fn handle_called_resp(&self, called_resp: Box<SnCalledResp>, _aes_key: Option<&AesKey>) {
        info!("called-resp seq {}.", called_resp.seq.value());
        self.resend_queue().confirm_pkg(called_resp.seq.value());

        // 统计性能
        // if let Some((call_send_time, called_send_time, peerid)) = self.call_tracker.calls.remove(&called_resp.seq) {
        //     if let Some(cached_peer) = self.peer_mgr.find_peer(&peerid, FindPeerReason::Other) {
        //         let now_time_stamp = bucky_time_now();
        //         if now_time_stamp > call_send_time {
        //             let call_delay = (now_time_stamp - call_send_time) / 1000;
        //             cached_peer.receipt.call_delay = ((cached_peer.receipt.call_delay as u64 * 7 + call_delay) / 8) as u16;
        //         }

        //         let rto = Instant::now().duration_since(called_send_time).as_millis() as u32;
        //         cached_peer.receipt.rto = ((cached_peer.receipt.rto as u32 * 7 + rto) / 8) as u16;
        //     }
        // }
    }
}
