use std::{
    sync::{Arc, RwLock,}, 
    collections::{BTreeMap},
    convert::TryFrom, 
    time::{Duration}
};
use async_std::{
    task
};

use cyfs_base::*;
use cyfs_debug::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{NetListener, UpdateOuterResult, udp::{Interface, PackageBoxEncodeContext}}, 
    history::keystore, 
    stack::{WeakStack, Stack} 
};
use super::super::types::*;
use super::{ 
    Config,
};

pub trait PingClientStateEvent: Send + Sync {
    fn online(&self, sn: &Device);
    fn offline(&self, sn: &Device);
}

pub trait PingClientCalledEvent<Context=()>: Send + Sync {
    fn on_called(&self, called: &SnCalled, context: Context) -> Result<(), BuckyError>;
}

pub trait PingClient {
    fn clone_as_ping_client(&self) -> Box<dyn PingClient>;
    fn status(&self) -> SnStatus;
    fn start(&self);
    fn stop(&self);
    fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface) -> BuckyResult<()>;
}


pub(crate) struct PingManager {
    stack: WeakStack, 
    clients: RwLock<BTreeMap<DeviceId, Box<dyn PingClient>>,
}

impl std::fmt::Display for PingManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack = Stack::from(&self.stack);
        write!(f, "PingManager{{local:{}}}", stack.local_device_id())
    }
}

impl PingManager {
    pub fn new(stack: WeakStack) -> PingManager {
        PingManager {
            stack, 
            clients: RwLock::new(Default::default()),
        }
    }

    fn client_of(&self, sn_id: &DeviceId) -> Option<Box<dyn PingClient>> {
        self.clients.read().unwrap().get(sn_id).map(|c| c.clone_as_ping_client())
    }

    pub fn status_of(&self, sn_id: &DeviceId) -> Option<SnStatus> {
        self.client_of(sn_id).map(|c| c.status())
    }

    fn new_client(net_listener: &NetListener, sn: Device) -> Box<dyn PingClient> {
        unimplemented!()
    }

    pub fn reset(&self, net_listener: NetListener, sn_list: Vec<Device>) {
        log::info!("{} starting.", self);

        let (to_start, to_stop) = {
            let mut clients = self.clients.write().unwrap();
            let to_stop: Vec<Client> = clients.values().map(|c| c.clone_as_ping_client()).collect();
            clients.clear();

            let mut to_start = vec![];
            for sn in sn_list {
                let sn_id = sn.desc().device_id();
                log::info!("{} add-sn: {}", self, sn_id.to_string());

                let client = Self::new_client(&net_listener, sn);
                clients.insert(sn_id, client.clone_as_ping_client());
                to_start.push(client);
            }
            (to_start, to_stop)
        };
               
        for client in to_stop {
            client.stop();
        }

        for client in to_start {
            client.start();
        }

        log::info!("{} started.", self);
    }

    pub fn resend_ping(&self) {
        let clients: Vec<Arc<Client>> = self.clients.read().unwrap().values().collect();
        for client in clients {
            client.local_updated();
            client.send_ping();
        }
    }

    fn on_ping_resp(&self) {

    }

    pub fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface) -> BuckyResult<()> {
        log::info!("{} ping-resp, sn: {}/{}, seq: {}.", self, resp.sn_peer_id.to_string(), from.to_string(), resp.seq.value());

        let client = self.clients.read().unwrap().get(&resp.sn_peer_id).map(|c| c.clone());
        let (new_endpoint, is_resend_immediate) = match client.as_ref() {
            None => {
                log::warn!("{} ping-resp, sn: {}/{} not found, maybe is stopped.", self, resp.sn_peer_id.to_string(), from.to_string());
                return Err(BuckyError::new(BuckyErrorCode::ErrorState, "the sn maybe is removed"));
            },
            Some(client) => {
                client.on_udp_ping_resp(resp, &from, from_interface)
            }
        };

        Ok(())
    }

    pub fn on_called(&self, called: &SnCalled, in_box: &PackageBox, from: &Endpoint, from_interface: Interface) -> BuckyResult<()> {
        if &called.to_peer_id != Stack::from(&self.stack).local_device_id() {
            log::warn!("{} called, recv called to other: {}.", self, called.to_peer_id.to_string());
            return Err(BuckyError::new(BuckyErrorCode::AddrNotAvailable, "called to other"));
        }

        log::info!("{} called, sn: {}, from: {}, seq: {}, from-eps: {}.",
            self, 
            called.sn_peer_id.to_string(),
            called.peer_info.desc().device_id().to_string(),
            called.seq.value(),
            called.peer_info.connect_info().endpoints().iter().map(|ep| ep.to_string()).collect::<Vec<String>>().concat());

        let client = self.clients.read().unwrap().get(&called.sn_peer_id).cloned();

        let stack = Stack::from(&self.env.stack);
        let called = called.clone();
        let key = in_box.key().clone();
        let from = from.clone();
        task::spawn(async move {
            let peer_info = &called.peer_info;
            if let Some(sigs) = peer_info.signs().body_signs() {
                if let Some(sig) = sigs.get(0) {
                    match verify_object_body_sign(&RsaCPUObjectVerifier::new(peer_info.desc().public_key().clone()), peer_info, sig).await {
                        Ok(is_ok) if is_ok => {},
                        _ => {
                            log::warn!("{} sn-called verify failed, from {:?}", stack.local_device_id(), from);
                            return;
                        }
                    }
                }
            }

            if let Ok(_) = PingClientCalledEvent::on_called(&stack, &called, ()) {
                match client {
                    None => log::warn!("{} the sn maybe is removed when recv called-req.", stack.local_device_id()),
                    Some(client) => {
                        client.on_called(&called, &key, called.call_seq, bucky_time_to_system_time(called.call_send_time), &from, from_interface);
                    }
                };
            }
        });
        Ok(())
    }
}




#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SnStatus {
    Connecting, 
    Online(Timestamp), 
    Offline
}



fn is_new_endpoint(desc: &Device, ep: &Endpoint) -> bool {
    for cur in desc.connect_info().endpoints() {
        if cur.is_udp() && cur.addr() == ep.addr() {
            return false;
        }
    }
    true
}
