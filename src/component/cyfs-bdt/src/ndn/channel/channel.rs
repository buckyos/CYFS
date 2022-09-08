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
    scheduler::*, 
    download::*
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
    pub udp: udp::Config, 
    pub history_speed: HistorySpeedConfig
}



struct ChannelActiveState {
    guard: TunnelGuard, 
    tunnel: DynamicChannelTunnel,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelState {
    Unknown, 
    Active, 
    Dead
}

enum StateImpl {
    Unknown, 
    Active(ChannelActiveState), 
    Dead(Option<Timestamp>)
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
            TaskState::Canceled(err) => {
                match err {
                    BuckyErrorCode::Redirect | BuckyErrorCode::Pending => {
                        info!("{} session need redirect or wait-redirect opertator", session.channel());
                    }, 
                    _ => {
                        let mut sessions = self.sessions.write().unwrap();
                        if sessions.canceled.iter().find(|s| session.session_id().eq(s.session_id())).is_none() {
                            info!("{} add canceled upload session {}", session.channel(), session);
                            sessions.canceled.push_back(session);
                        }
                    }
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


struct DownloadState {
    sessions: BTreeMap<TempSeq, DownloadSession>, 
    speed_counter: SpeedCounter, 
    history_speed: HistorySpeed, 
}
struct Downloaders(RwLock<DownloadState>);

impl Downloaders {
    fn new(history_speed: HistorySpeed) -> Self {
        Self(RwLock::new(DownloadState {
            sessions: BTreeMap::new(), 
            speed_counter: SpeedCounter::new(0), 
            history_speed
        }))
    }

    fn initial_speed(&self) -> u32 {
        let downloaders = self.0.read().unwrap();
        let session_count: u32 = downloaders.sessions.values().map(|session| {
            match session.state() {
                DownloadSessionState::Downloading(_) => 1, 
                _ => 0
            }
        }).sum();
        downloaders.history_speed.average() / (session_count + 1)
    }
   
    fn is_empty(&self) -> bool {
        self.0.read().unwrap().sessions.is_empty()
    }

    fn remove(&self, id: &TempSeq) {
        let _ = self.0.write().unwrap().sessions.remove(id);
    }

    fn find(&self, id: &TempSeq) -> Option<DownloadSession> {
        self.0.read().unwrap().sessions.get(id).cloned()
    }

    fn add(&self, session: DownloadSession) -> BuckyResult<()> {
        let mut downloaders = self.0.write().unwrap();
        let _ = if downloaders.sessions.get(session.session_id()).is_some() {
            Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "duplicated"))
        } else {
            downloaders.sessions.insert(session.session_id().clone(), session.clone());
            Ok(())
        }?;

        let _ = session.start();
        task::spawn(async move {
            let state = session.wait_finish().await;
            // 这里等待2*msl
            if match state {
                DownloadSessionState::Finished => {
                    true
                }, 
                DownloadSessionState::Canceled(err) => {
                    if err == BuckyErrorCode::Interrupted {
                        true 
                    } else {
                        false
                    }
                }, 
                _ => unreachable!()
            } {
                let _ = future::timeout(2 * session.channel().config().msl, future::pending::<()>()).await;
            }
            
            let _ = session.channel().0.downloaders.remove(session.session_id());
            debug!("{} remove session {}", session.channel(), session);
        });
        Ok(())
    } 

    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut downloaders = self.0.write().unwrap();

        let session_count: u32 = downloaders.sessions.values().map(|session| {
            match session.state() {
                DownloadSessionState::Downloading(_) => 1, 
                _ => 0
            }
        }).sum();
        let cur_speed = downloaders.speed_counter.update(when);
        if cur_speed > 0 || session_count > 0 {
            downloaders.history_speed.update(Some(cur_speed), when);
            cur_speed
        } else {
            downloaders.history_speed.update(None, when);
            0
        }
    }

    fn cur_speed(&self) -> u32 {
        self.0.read().unwrap().history_speed.latest()
    }
    
    fn history_speed(&self) -> u32 {
        self.0.read().unwrap().history_speed.average()
    }

    fn on_piece_data(&self, piece: &PieceData) -> BuckyResult<()> {
        if let Some(session) = {
            let mut downloaders = self.0.write().unwrap();
            downloaders.speed_counter.on_recv(piece.data.len());
            downloaders.sessions.get(&piece.session_id).cloned()
        } {
            session.push_piece_data(piece);
            Ok(())
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "no session"))
        }
    }

    fn on_time_escape(&self, now: Timestamp) -> bool {
        let mut income_dead = true;
        let downloaders: Vec<DownloadSession> = self.0.read().unwrap().sessions.values().cloned().collect();
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

        income_dead
    }

    fn cancel_by_error(&self, err: BuckyError) {
        let downloaders: Vec<DownloadSession> = self.0.read().unwrap().sessions.values().cloned().collect();
        for session in downloaders {
            session.cancel_by_error(BuckyError::new(err.code(), err.msg().to_string()));
        }
    }
}


struct ChannelImpl {
    config: Config, 
    stack: WeakStack, 
    remote: DeviceId, 
    command_tunnel: DatagramTunnelGuard, 
    command_seq: TempSeqGenerator,  
    downloaders: Downloaders, 
    uploaders: Uploaders, 
    state: RwLock<StateImpl>, 
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
        command_tunnel: DatagramTunnelGuard, 
        history_speed: HistorySpeed 
    ) -> Self {
        let stack = Stack::from(&weak_stack);
        let config = stack.config().ndn.channel.clone();
        Self(Arc::new(ChannelImpl {
            config, 
            stack: weak_stack, 
            remote, 
            command_tunnel, 
            command_seq: TempSeqGenerator::new(), 
            downloaders: Downloaders::new(history_speed), 
            uploaders: Uploaders::new(), 
            state: RwLock::new(StateImpl::Unknown), 
        }))
    }

    pub fn reset(&self) {
        assert!(self.0.uploaders.is_empty());
        assert!(self.0.downloaders.is_empty());
        *self.0.state.write().unwrap() = StateImpl::Unknown;
    }

    pub fn remote(&self) -> &DeviceId {
        &self.0.remote
    }

    pub fn config(&self) -> &Config {
        &self.0.config
    }

    pub fn upload(&self, session: UploadSession) -> BuckyResult<()> {
        self.0.uploaders.add(session);
        Ok(())
    }

    pub fn download(&self, session: DownloadSession) -> BuckyResult<()> {
        let _ = self.0.downloaders.add(session.clone()).map_err(|err| {
            debug!("{} add session {} failed for {}", self, session, err);
            err
        })?;

        debug!("{} add session {}", self, session);
        Ok(())
    } 

    pub(super) fn gen_command_seq(&self) -> TempSeq {
        self.0.command_seq.generate()
    }

    // 从 datagram tunnel 发送控制命令
    pub fn interest(&self, interest: Interest) {
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

    pub fn resp_interest(&self, resp: RespInterest) {
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

    pub fn stack(&self) -> Stack {
        Stack::from(&self.0.stack)
    }

    pub fn state(&self) -> ChannelState {
        let state = &*self.0.state.read().unwrap();
        match state {
            StateImpl::Unknown => ChannelState::Unknown,
            StateImpl::Active(_) => ChannelState::Active,
            StateImpl::Dead(_) => ChannelState::Dead,
        }
    }

    pub fn clear_dead(&self) {
        let state = &mut *self.0.state.write().unwrap();
        match state {
            StateImpl::Dead(_) => {
                info!("{} Dead=>Unknown", self);
                *state = StateImpl::Unknown;
            },
            _ => {},
        }
    }

    fn calc_speed(&self, when: Timestamp) {
        let _ = self.0.downloaders.calc_speed(when);
    }

    pub fn initial_download_session_speed(&self) -> u32 {
        self.0.downloaders.initial_speed()
    }

    fn download_cur_speed(&self) -> u32 {
        self.0.downloaders.cur_speed()
    }

    fn download_history_speed(&self) -> u32 {
        self.0.downloaders.history_speed()
    }


    fn tunnel(&self) -> Option<DynamicChannelTunnel> {
        let state = &*self.0.state.read().unwrap();
        match state {
            StateImpl::Active(active) => Some(active.tunnel.clone_as_tunnel()), 
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
            stack.ndn().event_handler().on_newly_interest(&self.stack(), command, self).await
        }
    }

    fn on_resp_interest(&self, command: &RespInterest) -> BuckyResult<()> {
        match command.err {
            BuckyErrorCode::NotConnected => {
                let to = command.to.as_ref().unwrap();
                if let Some(requestor) = self.stack().ndn().channel_manager().channel_of(to) {
                    requestor.resp_interest(RespInterest { session_id: command.session_id.clone(),
                                                                 chunk: command.chunk.clone(),
                                                                 err: BuckyErrorCode::Redirect,
                                                                 redirect: command.redirect.clone(),
                                                                 redirect_referer: command.redirect_referer.clone(),
                                                                 to: None });
                } else {
                    error!("{} not found requestor channel {}", self, to);
                }
                Ok(())
            },
            _ => {
                if let Some(session) = self.0.downloaders.find(&command.session_id) {
                    session.on_resp_interest(command)
                } else {
                    Ok(())
                }
            }
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
        if self.0.downloaders.on_piece_data(&piece).is_err() {
            let strong_stack = Stack::from(&self.0.stack);
            // 这里可能要保证同步到同线程处理,重入会比较麻烦
            match strong_stack.ndn().event_handler().on_unknown_piece_data(&self.stack(), &piece, self) {
                Ok(session) => {
                    session.push_piece_data(&piece);
                    //FIXME： 如果新建了任务，这里应当继续接受piece data
                },
                Err(err) => {
                    // 通过新建一个canceled的session来回复piece control
                    let session = DownloadSession::canceled(
                        self.0.stack.clone(), 
                        piece.chunk.clone(), 
                        piece.session_id.clone(), 
                        self.clone(),
                        err);
                    let _ = self.0.downloaders.add(session.clone());
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
                StateImpl::Unknown => None, 
                StateImpl::Active(active) => Some(active.tunnel.clone_as_tunnel()), 
                _ => {
                    return;
                }
            }
        };
       
        if self.0.downloaders.on_time_escape(now) {
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
            if let StateImpl::Active(active) = state {
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
                StateImpl::Unknown => {
                    Some("Unknown")
                }, 
                StateImpl::Active(active) => {
                    // do nothing
                    if let TunnelState::Active(_) = active.tunnel.state() {
                        unreachable!()
                    } else {
                        Some("Active")
                    }   
                }, 
                StateImpl::Dead(_) => {
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
                                *state = StateImpl::Active(ChannelActiveState {
                                    guard, 
                                    tunnel: tunnel.clone_as_tunnel(),
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
                StateImpl::Unknown => None, 
                StateImpl::Active(active) => Some((
                    active.guard.clone(), 
                    active.tunnel.start_at(), 
                    active.tunnel.active_timestamp())), 
                    StateImpl::Dead(_) => None
            };
            *state = StateImpl::Dead(tunnel_state.clone().map(|(_, _, r)| r));
            tunnel_state
        };

        self.0.downloaders.cancel_by_error(BuckyError::new(BuckyErrorCode::Timeout, "channel's dead"));
        self.0.uploaders.cancel_by_error(BuckyError::new(BuckyErrorCode::Timeout, "channel's dead"));


        if let Some((tunnel, start_at, remote_timestamp)) = tunnel_state {
            let _ = tunnel.mark_dead(remote_timestamp, start_at);
        }
    }

    pub(super) fn next_piece(&self, buf: &mut [u8]) -> usize {
        self.0.uploaders.next_piece(buf)
    }
}


