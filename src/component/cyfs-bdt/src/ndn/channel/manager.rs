use std::{
    sync::RwLock, 
    collections::BTreeMap, 
};
use async_std::{
    sync::Arc, 
    task, 
};
use cyfs_base::*;
use crate::{
    types::*, 
    interface::{udp::OnUdpRawData},
    tunnel::TunnelContainer, 
    datagram::{self, DatagramTunnelGuard},
    stack::{WeakStack, Stack}
};
use super::channel::Channel;

struct ManagerImpl {
    stack: WeakStack, 
    command_tunnel: DatagramTunnelGuard, 
    entries: RwLock<BTreeMap<DeviceId, Channel>>, 
}

#[derive(Clone)]
pub struct ChannelManager(Arc<ManagerImpl>);

impl std::fmt::Display for ChannelManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChannelManager")
    }
}

impl ChannelManager {
    pub fn new(weak_stack: WeakStack) -> Self {
        let stack = Stack::from(&weak_stack);
        let command_tunnel = stack.datagram_manager().bind_reserved(datagram::ReservedVPort::Channel).unwrap();
        let manager = Self(Arc::new(ManagerImpl {
            stack: weak_stack.clone(), 
            entries: RwLock::new(BTreeMap::new()), 
            command_tunnel
        }));
        
        {
            let manager = manager.clone();
            task::spawn(async move {
                manager.recv_command().await;
            });
        }

        manager
    }



    pub fn channel_of(&self, remote: &DeviceId) -> Option<Channel> {
        self.0.entries.read().unwrap().get(remote).cloned()
    }

    pub fn create_channel(&self, remote: &DeviceId) -> Channel {
        let mut entries = self.0.entries.write().unwrap();
        entries.get(remote).map(|c| c.clone()).map_or_else(|| {
            trace!("{} create channel on {}", self, remote);
            let channel = Channel::new(
                self.0.stack.clone(), 
                remote.clone(), 
                self.0.command_tunnel.clone());
            entries.insert(remote.clone(), channel.clone());

            channel
        }, |c| c)
    } 

    pub(crate) fn on_time_escape(&self, now: Timestamp) {
        let channels: Vec<Channel> = self.0.entries.read().unwrap().values().cloned().collect();
        for channel in channels {
            channel.on_time_escape(now);
        }
    }

    async fn recv_command(&self) {
        loop {
            match self.0.command_tunnel.recv_v().await {
                Ok(datagrams) => {
                    for datagram in datagrams {
                        let channel = self.channel_of(&datagram.source.remote)
                            .or_else(| | Some(self.create_channel(&datagram.source.remote))).unwrap();
                        let _ = channel.on_datagram(datagram);
                    }
                }, 
                Err(_err) => {
                    
                }
            }
        }
    }
}

impl OnUdpRawData<&TunnelContainer> for ChannelManager {
    fn on_udp_raw_data(&self, data: &[u8], tunnel: &TunnelContainer) -> Result<(), BuckyError> {
        let channel = self.channel_of(tunnel.remote()).ok_or_else(| | BuckyError::new(BuckyErrorCode::NotFound, "channel not exists"))?;
        channel.on_udp_raw_data(data, None)
    }
}