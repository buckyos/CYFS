use log::*;
use std::{
    sync::{Arc, RwLock,}, 
    collections::{LinkedList},
};

use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{NetListener, udp::{Interface, PackageBoxEncodeContext}}, 
    stack::{WeakStack, Stack},
    dht::*
};
use super::super::{
    manager::PingClientCalledEvent
};
use super::{
    udp::{self, UdpClient}
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SnStatus {
    Connecting, 
    Online(Timestamp), 
    Offline
}


pub(super) trait PingClient: Send + Sync {
    fn sn(&self) -> &DeviceId;
    fn clone_as_ping_client(&self) -> Box<dyn PingClient>;
    fn status(&self) -> SnStatus;
    fn start(&self);
    fn stop(&self);
    fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface) -> BuckyResult<()>;
}

#[derive(Clone)]
pub struct Config {
    pub udp: udp::Config
}

struct ManagerState {
    remain: LinkedList<DeviceId>, 
    client: Option<Box<dyn PingClient>>
}

struct ManagerImpl {
    stack: WeakStack, 
    net_listener: NetListener, 
    sn_list: Vec<Device>, 
    state: RwLock<ManagerState>
}

#[derive(Clone)]
pub struct PingManager(Arc<ManagerImpl>); 

impl std::fmt::Display for PingManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack = Stack::from(&self.0.stack);
        write!(f, "PingManager{{local:{}}}", stack.local_device_id())
    }
}

impl PingManager {
    pub(crate) fn new(stack: WeakStack, net_listener: NetListener, sn_list: Vec<Device>) -> Self {
        let mut remain = LinkedList::new();
        for sn_id in sn_list.iter().map(|d| d.desc().device_id()) {
            remain.push_back(sn_id);
        }
        let strong_stack = Stack::from(&stack);
        
        let client = if let Some(sn_id) = Self::next_sn(strong_stack.local_device_id(), &mut remain) {
            let sn = sn_list.iter().find(|d| d.desc().device_id().eq(&sn_id)).cloned().unwrap();
            let client = Self::new_client(&net_listener, sn);
            client.start();
            Some(client)
        } else {
            None
        };

        Self(Arc::new(ManagerImpl {
            stack, 
            net_listener, 
            sn_list, 
            state: RwLock::new(ManagerState {
                remain, 
                client: client
            }),
        }))
    }

    pub fn sn_list(&self) -> &Vec<Device> {
        &self.0.sn_list
    }

    pub fn status(&self) -> SnStatus {
        self.0.state.read().unwrap().client.as_ref().map(|c| c.status()).unwrap_or(SnStatus::Offline)
    }

    pub(crate) fn close(&self) {
        let to_stop = {
            let mut state = self.0.state.write().unwrap();
            let mut to_stop = None;
            std::mem::swap(&mut to_stop, &mut state.client);
            state.remain.clear();
            state.client = None;
            to_stop
        };

        if let Some(client) = to_stop {
            client.stop();
        }
    }

    fn device_of(&self, sn_id: &DeviceId) -> Device {
        self.0.sn_list.iter().find(|d| d.desc().device_id().eq(sn_id)).cloned().unwrap()
    }

    fn next_sn(local_id: &DeviceId, remain: &mut LinkedList<DeviceId>) -> Option<DeviceId> {
        if let Some(i) = remain.iter().enumerate().min_by(|(_, l), (_, r)| l.object_id().distance(local_id.object_id()).cmp(&r.object_id().distance(local_id.object_id()))).map(|(i, _)| i) {
            let mut last_part = remain.split_off(i);
            let sn_id = last_part.pop_front();
            remain.append(&mut last_part);
            sn_id
        } else {
            None   
        }
    }

    fn new_client(net_listener: &NetListener, sn: Device) -> Box<dyn PingClient> {
        unimplemented!()
    }

    fn client_of(&self, sn_id: &DeviceId) -> Option<Box<dyn PingClient>> {
        self.0.state.read().unwrap().client.as_ref().and_then(|c| if c.sn().eq(sn_id) { Some(c.clone_as_ping_client()) } else { None })
    }

    pub fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface) -> BuckyResult<()> {
        log::info!("{} ping-resp, sn: {}/{}, seq: {}.", self, resp.sn_peer_id.to_string(), from.to_string(), resp.seq.value());

        if let Some(client) = self.client_of(&resp.sn_peer_id) {
            client.on_udp_ping_resp(resp, &from, from_interface)
        } else {
            warn!("{} ping-resp, sn: {}/{} not found, maybe is stopped.", self, resp.sn_peer_id.to_string(), from.to_string());
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, "the sn maybe is removed"));
        }
    }

    pub fn on_called(&self, called: &SnCalled, in_box: &PackageBox, from: &Endpoint, from_interface: Interface) -> BuckyResult<()> {
        info!("{} called, called: {:?}", self, called);
        let stack = Stack::from(&self.0.stack);

        if !called.to_peer_id.eq(stack.local_device_id()) {
            warn!("{} called, recv called to other: {}.", self, called.to_peer_id);
            return Err(BuckyError::new(BuckyErrorCode::AddrNotAvailable, "called to other"));
        }
        if self.client_of(&called.sn_peer_id).is_none() {
            warn!("{} the sn maybe is removed when recv called-req. from {}", self, called.to_peer_id);
            return Err(BuckyError::new(BuckyErrorCode::AddrNotAvailable, "called to other"));
        }

        let resp = SnCalledResp {
            seq: called.seq,
            result: 0,
            sn_peer_id: called.sn_peer_id.clone(),
        };

        let mut pkg_box = PackageBox::encrypt_box(resp.sn_peer_id.clone(), in_box.key().clone());
        pkg_box.push(resp);

        let mut context = PackageBoxEncodeContext::default();
        let _ = from_interface.send_box_to(&mut context, &pkg_box, from);

        stack.on_called(&called, ());

        Ok(())
    }
}




