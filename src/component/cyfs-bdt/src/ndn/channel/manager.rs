use std::{
    sync::RwLock, 
    collections::{LinkedList, BTreeMap}, 
};
use async_std::{
    sync::Arc, 
    task, 
};
use cyfs_base::*;
use crate::{
    types::*, 
    tunnel::*, 
    datagram::{self, DatagramTunnelGuard},
    stack::{WeakStack, Stack}
};
use super::super::{
    types::*
};
use super::{
    channel::{Channel},
};

struct ChannelGuard {
    reserving: Option<Timestamp>, 
    channel: Channel
}

impl ChannelGuard {
    fn get(&self) -> Channel {
        self.channel.clone()
    }

    fn check(&mut self, when: Timestamp) -> bool {
        if self.channel.ref_count() > 1 {
            self.reserving = None;
            true
        } else if let Some(expire_at) = self.reserving {
            if when > expire_at {
                info!("{} expired at {}", self.channel, expire_at);
                false
            } else {
                true
            }
        } else {
            self.reserving = Some(when + self.channel.config().reserve_timeout.as_micros() as u64);
            true
        }
    }
}

struct Channels {
    download_history_speed: HistorySpeed, 
    download_cur_speed: u32, 
    upload_history_speed: HistorySpeed, 
    upload_cur_speed: u32, 
    entries: BTreeMap<DeviceId, ChannelGuard>, 
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
        let stack = Stack::from(&self.0.stack);
        write!(f, "ChannelManager:{{local:{}}}", stack.local_device_id())
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
                upload_history_speed: HistorySpeed::new(0, stack.config().ndn.channel.history_speed.clone()), 
                upload_cur_speed: 0, 
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


    pub(crate) fn on_statistic(&self) -> String {
        let channels = self.0.channels.read().unwrap();
        let mut download_session_count = 0;
        let mut upload_session_count = 0;
        let mut channel_count = 0;
        for (_, guard) in &channels.entries {
            channel_count += 1;
            download_session_count += guard.channel.download_session_count();
            upload_session_count += guard.channel.upload_session_count();
        }

        format!("ChannelCount: {}, UploadSessionCount:{}, DownloadSessionCount:{}", channel_count, upload_session_count, download_session_count)
    }

    pub fn channel_of(&self, remote: &DeviceId) -> Option<Channel> {
        self.0.channels.read().unwrap().entries.get(remote).map(|guard| guard.get())
    }

    pub fn create_channel(&self, remote_const: &DeviceDesc) -> BuckyResult<Channel> {
        let stack = Stack::from(&self.0.stack);
        let remote = remote_const.device_id();
        let tunnel = stack.tunnel_manager().create_container(remote_const)?;
        let mut channels = self.0.channels.write().unwrap();

        Ok(channels.entries.get_mut(&remote).map(|guard| guard.get())
        .map_or_else(|| {
            info!("{} create channel on {}", self, remote);

            let channel = Channel::new(
                self.0.stack.clone(), 
                tunnel, 
                self.0.command_tunnel.clone());
            channels.entries.insert(remote, ChannelGuard { reserving: None, channel: channel.clone() });

            channel
        }, |c| c))
    } 

    pub fn on_schedule(&self, when: Timestamp) {
        let mut channels = self.0.channels.write().unwrap();
        let mut download_cur_speed = 0;
        let mut download_session_count = 0;
        let mut upload_cur_speed = 0;
        let mut upload_session_count = 0;

        let mut remove = LinkedList::new();
        for (remote, guard) in &mut channels.entries {
            let (d, u) = guard.channel.calc_speed(when);
            download_cur_speed += d;
            upload_cur_speed += u;

            download_session_count += guard.channel.download_session_count();
            upload_session_count += guard.channel.upload_session_count();

            if !guard.check(when) {
                remove.push_back(remote.clone());
            }
        }
        for remote in remove {
            info!("{} will remove channel for not used, channel={}", self, remote);
            channels.entries.remove(&remote);
        }

        channels.download_cur_speed = download_cur_speed;
        channels.upload_cur_speed = upload_cur_speed;

        if download_session_count > 0 {
            channels.download_history_speed.update(Some(download_cur_speed), when);
        } else {
            channels.download_history_speed.update(None, when);
        }

        if upload_session_count > 0 {
            channels.upload_history_speed.update(Some(upload_cur_speed), when);
        } else {
            channels.upload_history_speed.update(None, when);
        }

    }

    fn download_cur_speed(&self) -> u32 {
        self.0.channels.read().unwrap().download_cur_speed
    }

    fn download_history_speed(&self) -> u32 {
        self.0.channels.read().unwrap().download_history_speed.average()
    }

    fn upload_cur_speed(&self) -> u32 {
        self.0.channels.read().unwrap().upload_cur_speed
    }

    fn upload_history_speed(&self) -> u32 {
        self.0.channels.read().unwrap().upload_history_speed.average()
    }

    pub(crate) fn on_time_escape(&self, now: Timestamp) {
        let channels: Vec<Channel> = self.0.channels.read().unwrap().entries.values().map(|guard| guard.get()).collect();
        for channel in channels {
            channel.on_time_escape(now);
        }
    }

    async fn recv_command(&self) {
        let stack = Stack::from(&self.0.stack);
        loop {
            match self.0.command_tunnel.recv_v().await {
                Ok(datagrams) => {
                    for datagram in datagrams {
                        let channel = if let Some(channel) = self.channel_of(&datagram.source.remote) {
                            channel
                        } else {
                            let tunnel = stack.tunnel_manager().container_of(&datagram.source.remote).unwrap();
                            self.create_channel(tunnel.remote_const()).unwrap()
                        };
                        let _ = channel.on_datagram(datagram);
                    }
                }, 
                Err(_err) => {
                    
                }
            }
        }
    }

    pub fn on_raw_data(&self, data: &[u8], context: (&TunnelContainer, DynamicTunnel)) -> Result<(), BuckyError> {
        let (container, tunnel) = context;
        let channel = self.channel_of(container.remote()).ok_or_else(| | BuckyError::new(BuckyErrorCode::NotFound, "channel not exists"))?;
        channel.on_raw_data(data, tunnel)
    }
}
