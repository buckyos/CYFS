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
use super::super::{
    download::*
};
use super::{
    channel::Channel,
};

struct Channels {
    download_history_speed: HistorySpeed, 
    download_cur_speed: u32, 
    entries: BTreeMap<DeviceId, Channel>, 
}

struct ManagerImpl {
    stack: WeakStack, 
    command_tunnel: DatagramTunnelGuard, 
    channels: RwLock<Channels>
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
            command_tunnel, 
            channels: RwLock::new(Channels {
                download_history_speed: HistorySpeed::new(0, stack.config().ndn.channel.history_speed.clone()), 
                download_cur_speed: 0, 
                entries: BTreeMap::new()
            }), 
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
        self.0.channels.read().unwrap().entries.get(remote).cloned()
    }

    pub fn create_channel(&self, remote: &DeviceId) -> Channel {
        let mut channels = self.0.channels.write().unwrap();

        channels.entries.get(remote).map(|c| c.clone()).map_or_else(|| {
            info!("{} create channel on {}", self, remote);
            let initial_download_speed = channels.download_history_speed.average() / (channels.entries.len() as u32 + 1);
            let channel = Channel::new(
                self.0.stack.clone(), 
                remote.clone(), 
                self.0.command_tunnel.clone(), 
                HistorySpeed::new(initial_download_speed, channels.download_history_speed.config().clone())
            );
            channels.entries.insert(remote.clone(), channel.clone());

            channel
        }, |c| c)
    } 

    pub fn calc_speed(&self, when: Timestamp) {
        let mut channels = self.0.channels.write().unwrap();
        let mut download_cur_speed = 0;
        let mut download_session_count = 0;
        for channel in channels.entries.values() {
            let d = channel.calc_speed(when);
            download_cur_speed += d;
            download_session_count += channel.download_session_count();
        }
        channels.download_cur_speed = download_cur_speed;
        if download_session_count > 0 {
            channels.download_history_speed.update(Some(download_cur_speed), when);
        } else {
            channels.download_history_speed.update(None, when);
        }
    }

    fn download_cur_speed(&self) -> u32 {
        self.0.channels.read().unwrap().download_cur_speed
    }

    fn download_history_speed(&self) -> u32 {
        self.0.channels.read().unwrap().download_history_speed.average()
    }

    pub(crate) fn on_time_escape(&self, now: Timestamp) {
        let channels: Vec<Channel> = self.0.channels.read().unwrap().entries.values().cloned().collect();
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