use log::*;
use std::{
    convert::TryFrom, 
    sync::{RwLock, atomic::{AtomicU64, Ordering}},
    collections::{BTreeMap, LinkedList}, 
    time::Duration, 
};
use async_std::{
    sync::Arc, 
    task, 
    future
};
use cyfs_base::*;
use crate::{
    types::*, 
    interface::udp::{OnUdpRawData, MTU}, 
    protocol::*, 
    tunnel::{TunnelGuard, TunnelState}, 
    datagram::{self, DatagramTunnelGuard, Datagram, DatagramOptions}, 
    stack::{WeakStack, Stack}
};
use super::super::{
    scheduler::*
};
use super::{
    download::*, 
    upload::*, 
    protocol::v0::*, 
    tunnel::*,
    
};


#[derive(Clone)]
pub struct Config { 
    pub precoding_timeout: Duration,
    pub resend_interval: Duration, 
    pub resend_timeout: Duration,  
    pub wait_redirect_timeout: Duration,
    pub msl: Duration, 
    pub udp: udp::Config
}



struct ChannelActiveState {
    guard: TunnelGuard, 
    tunnel: DynamicChannelTunnel,
    statistic_task: DynamicStatisticTask,
}

enum ChannelState {
    Unknown, 
    Active(ChannelActiveState), 
    Dead(Option<Timestamp>)
}

pub enum ChannelConnectionState {
    Unknown,
    Active,
    Dead(Option<Timestamp>),
}

struct UploadSessions {
    uploading: Vec<UploadSession>, 
    canceled: LinkedList<UploadSession>
} 

struct Uploaders {
    sessions: RwLock<UploadSessions>, 
    piece_seq: AtomicU64, 
}


impl Uploaders {
    fn new() -> Self {
        Self {
            sessions: RwLock::new(UploadSessions {
                uploading: vec![], 
                canceled: LinkedList::new()
            }), 
            piece_seq: AtomicU64::new(0), 
        }
    }

    fn is_empty(&self) -> bool {
        let sessions = self.sessions.read().unwrap();
        sessions.canceled.is_empty() && sessions.uploading.is_empty()
    } 

    fn find(&self, session_id: &TempSeq) -> Option<UploadSession> {
        let sessions = self.sessions.read().unwrap();
        sessions.uploading.iter().find(|session| session.session_id().eq(session_id))
            .or_else(|| sessions.canceled.iter().find(|session| session.session_id().eq(session_id))).cloned()
    }

    fn add(&self, session: UploadSession) {
        match session.schedule_state() {
            TaskState::Canceled(_) => {
                let mut sessions = self.sessions.write().unwrap();
                if sessions.canceled.iter().find(|s| session.session_id().eq(s.session_id())).is_none() {
                    info!("{} add canceled upload session {}", session.channel(), session);
                    sessions.canceled.push_back(session);
                }
            },  
            _ => {
                let mut sessions = self.sessions.write().unwrap();
                if sessions.uploading.iter().find(|s| session.session_id().eq(s.session_id())).is_none() {
                    info!("{} add upload session {}", session.channel(), session);
                    sessions.uploading.push(session);
                }
            }
        }
    }

    fn remove(&self, session_id: &TempSeq) -> Option<UploadSession> {
        let mut sessions = self.sessions.write().unwrap();
        if let Some((i, _)) = sessions.uploading.iter().enumerate().find(|(_, session)| session_id.eq(session.session_id())) {
            let session = sessions.uploading.remove(i);
            info!("{} remove {}", session.channel(), session);
            Some(session)
        } else {
            None
        }
    }

    fn cancel_by_error(&self, err: BuckyError) {
        let uploading = self.sessions.read().unwrap().uploading.clone();
        for session in &uploading {
            session.cancel_by_error(BuckyError::new(err.code(), err.msg().to_string()));
        }
        let mut sessions = self.sessions.write().unwrap();
        for session in uploading {
            if let Some((i, _)) = sessions.uploading.iter().enumerate().find(|(_, s)| session.session_id().eq(s.session_id())) {
                let _ = sessions.uploading.remove(i);
                sessions.canceled.push_back(session);
            }
        }
    }

    fn next_piece(&self, buf: &mut [u8]) -> usize {
        let mut try_count = 0;
        loop {
            let ret = {
                let sessions = self.sessions.read().unwrap();
                if sessions.uploading.len() > 0 {
                    let seq = self.piece_seq.fetch_add(1, Ordering::SeqCst);
                    let index = (seq % (sessions.uploading.len() as u64)) as usize;
                    Some((sessions.uploading.get(index).unwrap().clone(), sessions.uploading.len()))
                } else {
                    None
                }
            };
            
            if let Some((session, session_count)) = ret {
                match session.next_piece(buf) {
                    Ok(len) => {
                        if len > 0 {
                            break len;
                        } else {
                            try_count += 1;
                            if try_count >= session_count {
                                break 0;
                            }
                        }
                    },
                    Err(err) => {
                        debug!("{} cancel {} for next piece failed for {}", session.channel(), session, err);
                        {   
                            let mut sessions = self.sessions.write().unwrap();
                            if let Some((i, _)) = sessions.uploading.iter().enumerate().find(|(_, s)| session.session_id().eq(s.session_id())) {
                                let _ = sessions.uploading.remove(i);
                                info!("{} remove {}", session.channel(), session);
                                sessions.canceled.push_back(session.clone());
                            }
                        }
                        try_count += 1;
                        if try_count >= session_count {
                            break 0;
                        }
                    }
                }
            } else {
                break 0;
            }
        } 
    }

    fn on_time_escape(&self, now: Timestamp) {
        let mut sessions = self.sessions.write().unwrap();

        let mut uploading = vec![];
        std::mem::swap(&mut sessions.uploading, &mut uploading);
        
        for session in uploading {
            if let Some(state) = session.on_time_escape(now) {
                match state {
                    TaskState::Finished => {
                        sessions.canceled.push_back(session);
                    },
                    TaskState::Canceled(_) => {
                        sessions.canceled.push_back(session);
                    }, 
                    _ => {
                        sessions.uploading.push(session);
                    }
                }
            } else {
                info!("{} remove session {}", session.channel(), session);
            }
        }

        let mut canceled = LinkedList::new();
        std::mem::swap(&mut sessions.canceled, &mut canceled);
        for session in canceled {
            if let Some(_) = session.on_time_escape(now) {
                // do nothing
            } else {
                info!("{} remove session {}", session.channel(), session);
            }
        }
    }
}

struct ChannelImpl {
    config: Config, 
    stack: WeakStack, 
    remote: DeviceId, 
    command_tunnel: DatagramTunnelGuard, 
    command_seq: TempSeqGenerator,  
    downloaders: RwLock<BTreeMap<TempSeq, DownloadSession>>, 
    uploaders: Uploaders, 
    state: RwLock<ChannelState>, 
    statistic_task: DynamicStatisticTask,
}

#[derive(Clone)]
pub struct Channel(Arc<ChannelImpl>);

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Channel{{local:{}, remote:{}}}", Stack::from(&self.0.stack).local_device_id(), self.remote())
    }
}

impl Channel {
    pub fn new(
        weak_stack: WeakStack, 
        remote: DeviceId, 
        command_tunnel: DatagramTunnelGuard) -> Self {
        let stack = Stack::from(&weak_stack);
        let config = stack.config().ndn.channel.clone();
        Self(Arc::new(ChannelImpl {
            config, 
            stack: weak_stack, 
            remote, 
            command_tunnel, 
            command_seq: TempSeqGenerator::new(), 
            downloaders: RwLock::new(BTreeMap::new()), 
            uploaders: Uploaders::new(), 
            state: RwLock::new(ChannelState::Unknown), 
            statistic_task: DynamicStatisticTask::default(),
        }))
    }

    pub fn reset(&self) {
        assert!(self.0.uploaders.is_empty());
        assert!(self.0.downloaders.read().unwrap().is_empty());
        *self.0.state.write().unwrap() = ChannelState::Unknown;
    }

    pub fn remote(&self) -> &DeviceId {
        &self.0.remote
    }

    pub fn config(&self) -> &Config {
        &self.0.config
    }

    pub fn download(&self, session: DownloadSession) -> BuckyResult<()> {
        {
            let mut downloaders = self.0.downloaders.write().unwrap();
            let _ = if downloaders.get(session.session_id()).is_some() {
                debug!("{} ignore session {} for duplicated", self, session);
                Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "duplicated"))
            } else {
                downloaders.insert(session.session_id().clone(), session.clone());
                debug!("{} add session {}", self, session);
                Ok(())
            }?;
        }

        let r = session.start();
        task::spawn(async move {
            let state = session.wait_finish().await;
            // 这里等待2*msl
            if match state {
                TaskState::Finished => {
                    true
                }, 
                TaskState::Canceled(err) => {
                    if err == BuckyErrorCode::Interrupted {
                        true 
                    } else {
                        false
                    }
                }, 
                TaskState::Redirect(_redirect_node, _redirect_referer) => {
                    // redirect
                    true
                },
                _ => unreachable!()
            } {
                let _ = future::timeout(2 * session.channel().config().msl, future::pending::<()>()).await;
            }
            
            let channel = session.channel();
            let _ = channel.0.downloaders.write().unwrap().remove(session.session_id());
            debug!("{} remove session {}", session.channel(), session);
        });
        assert!(r.is_ok());
        Ok(())
    } 

    pub(super) fn gen_command_seq(&self) -> TempSeq {
        self.0.command_seq.generate()
    }

    // 从 datagram tunnel 发送控制命令
    pub(in crate::ndn) fn interest(&self, interest: Interest) {
        let mut buf = vec![0u8; MTU];
        let mut options = DatagramOptions::default();
        let tail = interest.raw_encode_with_context(
            buf.as_mut_slice(), 
            &mut options, 
            &None).unwrap();
        let len = MTU - tail.len();
        let _ = self.0.command_tunnel.send_to(
            &buf[..len], 
            &mut options, 
            self.remote(), 
            datagram::ReservedVPort::Channel as u16);

    } 

    pub(in crate::ndn) fn resp_interest(&self, resp: RespInterest) {
        debug!("{} will send resp interest {:?}", self, resp);
        let mut buf = vec![0u8; MTU];
        let mut options = DatagramOptions::default();
        let tail = resp.raw_encode_with_context(
            buf.as_mut_slice(), 
            &mut options, 
            &None).unwrap();
        let len = MTU - tail.len();
        let _ = self.0.command_tunnel.send_to(
            &buf[..len], 
            &mut options, 
            self.remote(), 
            datagram::ReservedVPort::Channel as u16);
    }

    
    // 明文tunnel发送PieceControl
    pub(super) fn send_piece_control(&self, control: PieceControl) {
        if let Some(tunnel) = self.tunnel() {
            debug!("{} will send piece control {:?}", self, control);
            tunnel.send_piece_control(control);
        } else {
            debug!("{} ignore send piece control {:?} for channel dead", self, control);
        }
    }

    pub(super) fn on_datagram(&self, datagram: Datagram) -> BuckyResult<()> {
        if let Some(_) = self.active() {
            let (command_code, buf) = u8::raw_decode(datagram.data.as_ref())?;
            let command_code = CommandCode::try_from(command_code)?;
            match command_code {
                CommandCode::Interest => {
                    let (interest, _) = Interest::raw_decode_with_context(buf, &datagram.options)?;
                    let channel = self.clone();
                    task::spawn(async move {
                        let _ = channel.on_interest(&interest).await;
                    });
                    Ok(())
                }, 
                CommandCode::RespInterest => {
                    let (resp_interest, _) = RespInterest::raw_decode_with_context(buf, &datagram.options)?;
                    self.on_resp_interest(&resp_interest)
                }, 
                // _ => Err(BuckyError::new(BuckyErrorCode::InvalidInput, "invalid command"))
            }
        } else {
            Err(BuckyError::new(BuckyErrorCode::ErrorState, "channel's dead"))
        }
    }

    fn stack(&self) -> Stack {
        Stack::from(&self.0.stack)
    }

    pub fn statistic_task(&self) -> DynamicStatisticTask {
        return self.0.statistic_task.clone();
    }

    pub fn connection_state(&self) -> ChannelConnectionState {
        let state = &*self.0.state.read().unwrap();
        match state {
            ChannelState::Unknown => ChannelConnectionState::Unknown,
            ChannelState::Active(_) => ChannelConnectionState::Active,
            ChannelState::Dead(t) => ChannelConnectionState::Dead(*t),
        }
    }

    pub fn clear_dead(&self) {
        let state = &mut *self.0.state.write().unwrap();
        match state {
            ChannelState::Dead(_) => {
                info!("{} Dead=>Unknown", self);
                *state = ChannelState::Unknown;
            },
            _ => {},
        }
    }

    fn tunnel(&self) -> Option<DynamicChannelTunnel> {
        let state = &*self.0.state.read().unwrap();
        match state {
            ChannelState::Active(active) => Some(active.tunnel.clone_as_tunnel()), 
            _ => None
        }
    }

    async fn on_interest(&self, command: &Interest) -> BuckyResult<()> {
        info!("{} got interest {:?}", self, command);
        // 如果已经存在上传 session，什么也不干
        let session = self.0.uploaders.find(&command.session_id);
        
        if let Some(session) = session {
            info!("{} ignore {:?} for upload session exists", self, command);
            session.on_interest(command)
        } else {
            let stack = self.stack();
            let session = stack.ndn().event_handler().on_newly_interest(command, self).await?;
            // 加入到channel的 upload sessions中
            self.0.uploaders.add(session.clone());
            session.on_interest(command)
        }
    }

    fn on_resp_interest(&self, command: &RespInterest) -> BuckyResult<()> {
        if let Some(session) = self.0.downloaders.read().unwrap().get(&command.session_id).clone() {
            session.on_resp_interest(command)
        } else {
            Ok(())
        }
    }
}

impl OnUdpRawData<Option<()>> for Channel {
    fn on_udp_raw_data(&self, data: &[u8], _: Option<()>) -> BuckyResult<()> {
        if let Some(tunnel) = self.active() {
            let (cmd_code, buf) = u8::raw_decode(data)?;
            let cmd_code = PackageCmdCode::try_from(cmd_code)?;
            match cmd_code {
                PackageCmdCode::PieceData => {
                    let piece = PieceData::decode_from_raw_data(buf)?;
                    let _ = tunnel.on_piece_data(&piece)?;
                    self.on_piece_data(piece)
                }, 
                PackageCmdCode::PieceControl => {
                    let (mut ctrl, _) = PieceControl::raw_decode(buf)?;
                    let _ = tunnel.on_piece_control(&mut ctrl)?;
                    self.on_piece_control(&ctrl) 
                },
                PackageCmdCode::ChannelEstimate => {
                    let (est, _) = ChannelEstimate::raw_decode(buf)?;
                    tunnel.on_resp_estimate(&est) 
                }
                _ => unreachable!()
            }
        } else {
            Err(BuckyError::new(BuckyErrorCode::ErrorState, "channel's dead"))
        }
    }   
}

impl Channel {
    fn on_piece_data(&self, piece: PieceData) -> BuckyResult<()> {
        trace!("{} got piece data est_seq:{:?} chunk:{} desc:{:?} data:{}", self, piece.est_seq, piece.chunk, piece.desc, piece.data.len());

        let _ = self.0.statistic_task.on_stat(piece.data.len() as u64);

        if let Some(session) = self.0.downloaders.read().unwrap().get(&piece.session_id).clone() {
            if let Some(view) = Stack::from(&self.0.stack).ndn().chunk_manager().view_of(session.chunk()) {
                let _ = view.on_piece_stat(&piece);
            }
            session.push_piece_data(&piece);
        } else {
            let stack = Stack::from(&self.0.stack);
            // 这里可能要保证同步到同线程处理,重入会比较麻烦
            match stack.ndn().event_handler().on_unknown_piece_data(&piece, self) {
                Ok(_session) => {
                    unimplemented!()
                    //FIXME： 如果新建了任务，这里应当继续接受piece data
                },
                Err(err) => {
                    // 通过新建一个canceled的session来回复piece control
                    let session = DownloadSession::canceled(
                        piece.chunk.clone(), 
                        piece.session_id.clone(), 
                        self.clone(),
                        err);
                    let _ = self.download(session.clone());
                    session.push_piece_data(&piece);
                }  
            }
        }
        Ok(())
    }

    fn on_piece_control(&self, ctrl: &PieceControl) -> BuckyResult<()> {
        debug!("{} got piece control {:?}", self, ctrl);

        if let Some(session) = match ctrl.command {
            PieceControlCommand::Finish => {
                self.0.uploaders.remove(&ctrl.session_id)
            }, 
            PieceControlCommand::Cancel => {
                self.0.uploaders.remove(&ctrl.session_id)
            }, 
            PieceControlCommand::Continue => {
                self.0.uploaders.find(&ctrl.session_id) 
            },
            _ => unreachable!() 
        } {
            session.on_piece_control(ctrl)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "session not found"))
        }
    }

    pub fn on_time_escape(&self, now: Timestamp) {
        self.0.uploaders.on_time_escape(now);
        let tunnel = {
            let state = &*self.0.state.read().unwrap();
            match state {
                ChannelState::Unknown => None, 
                ChannelState::Active(active) => Some(active.tunnel.clone_as_tunnel()), 
                _ => {
                    return;
                }
            }
        };
       
        let mut income_dead = true;
        let downloaders: Vec<DownloadSession> = self.0.downloaders.read().unwrap().values().cloned().collect();
        if downloaders.len() == 0 {
            income_dead = false;
        } else {
            for d in downloaders {
                match d.on_time_escape(now) {
                    Ok(_) => {
                        income_dead = false;
                    },
                    _ => {}
                }
            }
        }

        if income_dead {
            error!("income break, channel:{}", self);
            self.mark_dead();
            return ;
        }

        if tunnel.is_none() {
            return ;
        }
        let tunnel = tunnel.unwrap();
        
        match tunnel.on_time_escape(now) {
            Ok(_) => {},
            Err(err) => {
                error!("tunnel break, channel:{}, err:{}", self, err);
                self.mark_dead();
            }
        }
    }

    fn active(&self) -> Option<DynamicChannelTunnel> {
        {
            let stack = self.stack();
            let default_tunnel;
            if let Some(tunnel) = stack.tunnel_manager().container_of(self.remote()) {
                if let Ok(t) = tunnel.default_tunnel() {
                    default_tunnel = t;
                } else {
                    error!("{} ignore active on dead tunnel", self);
                    return None;
                }
            } else {
                error!("{} ignore active on dead tunnel", self);
                return None;
            }
           
            let state = &*self.0.state.read().unwrap();
            if let ChannelState::Active(active) = state {
                if let TunnelState::Active(_) = active.tunnel.state() {
                    if active.tunnel.raw_ptr_eq(&default_tunnel) {
                        return Some(active.tunnel.clone_as_tunnel());
                    } else {
                        info!("{} will drop tunnel {}", self, active.tunnel);
                    }
                } else {
                    info!("{} will drop tunnel {}", self, active.tunnel);
                }
            }
        }

        let former_state = {
            match &*self.0.state.read().unwrap() {
                ChannelState::Unknown => {
                    Some("Unknown")
                }, 
                ChannelState::Active(active) => {
                    // do nothing
                    if let TunnelState::Active(_) = active.tunnel.state() {
                        unreachable!()
                    } else {
                        Some("Active")
                    }   
                }, 
                ChannelState::Dead(_) => {
                    Some("Dead")
                }
            }
        };


        if let Some(former_state) = former_state
        {
            let stack = Stack::from(&self.0.stack);
            let guard = stack.tunnel_manager().container_of(self.remote()).unwrap();
            match guard.default_tunnel() {
                Ok(raw_tunnel) => {
                    match new_channel_tunnel(self.clone(), raw_tunnel) {
                        Ok(tunnel) => {
                            {
                                let state = &mut *self.0.state.write().unwrap();
                                *state = ChannelState::Active(ChannelActiveState {
                                    guard, 
                                    tunnel: tunnel.clone_as_tunnel(),
                                    statistic_task: self.0.statistic_task.clone(),
                                });
                            }
                            
                            info!("{} {}=>Active{{tunnel:{}}}", self, former_state, tunnel);
                            Some(tunnel)
                        }, 
                        Err(err) => {
                            info!("{} ignore active for {}", self, err);
                            None
                        }
                    } 
                }, 
                Err(err) => {
                    info!("{} ignore active for {}", self, err);
                    None
                }
            }
        } else {
            None
        }
    }

    fn mark_dead(&self) {
        error!("channel dead, channel:{}", self);
        let tunnel_state = {
            let state = &mut *self.0.state.write().unwrap();
            let tunnel_state = match state {
                ChannelState::Unknown => None, 
                ChannelState::Active(active) => Some((
                    active.guard.clone(), 
                    active.tunnel.start_at(), 
                    active.tunnel.active_timestamp())), 
                ChannelState::Dead(_) => None
            };
            *state = ChannelState::Dead(tunnel_state.clone().map(|(_, _, r)| r));
            tunnel_state
        };

        let downloaders: Vec<DownloadSession> = self.0.downloaders.read().unwrap().values().cloned().collect();
        for session in downloaders {
            session.cancel_by_error(BuckyError::new(BuckyErrorCode::Timeout, "channel's dead"));
        }
        self.0.uploaders.cancel_by_error(BuckyError::new(BuckyErrorCode::Timeout, "channel's dead"));


        if let Some((tunnel, start_at, remote_timestamp)) = tunnel_state {
            let _ = tunnel.mark_dead(remote_timestamp, start_at);
        }
    }

    pub(super) fn next_piece(&self, buf: &mut [u8]) -> usize {
        self.0.uploaders.next_piece(buf)
    }
}


