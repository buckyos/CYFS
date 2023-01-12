use log::*;
use std::{
    convert::TryFrom, 
    sync::{RwLock},
    collections::{BTreeMap, LinkedList}, 
    time::Duration, 
};
use async_std::{
    sync::{Arc, Weak}, 
    task, 
};
use cyfs_base::*;
use crate::{
    types::*, 
    interface::udp::{MTU}, 
    protocol::*, 
    tunnel::{TunnelGuard, DynamicTunnel, TunnelState}, 
    datagram::{self, DatagramTunnelGuard, Datagram, DatagramOptions}, 
    stack::{WeakStack, Stack}
};
use super::super::{
    types::*, 
    chunk::*, 
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
    pub resend_interval: Duration, 
    pub resend_timeout: Duration,  
    pub block_interval: Duration,
    pub msl: Duration, 
    pub udp: udp::Config, 
    pub history_speed: HistorySpeedConfig, 
    pub reserve_timeout: Duration
}


struct StateImpl {
    download: DownloadState, 
    upload: UploadState, 
    tunnels: Vec<DynamicChannelTunnel>
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelState {
    Unknown, 
    Active, 
    Dead
}


struct DownloadState {
    running: BTreeMap<TempSeq, DownloadSession>, 
    canceled: BTreeMap<TempSeq, (DownloadSession, Timestamp)>, 
    speed_counter: SpeedCounter, 
    cur_speed: u32, 
    history_speed: HistorySpeed, 
}



impl DownloadState {
    fn new(history_speed: HistorySpeed) -> Self {
        Self {
            running: BTreeMap::new(), 
            canceled: BTreeMap::new(), 
            speed_counter: SpeedCounter::new(0), 
            cur_speed: 0, 
            history_speed
        }
    }
   
    fn is_empty(&self) -> bool {
        self.running.is_empty()
    }

    fn cancel(&mut self, id: &TempSeq) {
        if let Some(session) = self.running.remove(id) {
            self.canceled.insert(id.clone(), (session, bucky_time_now()));
        }
    }

    fn remove(&mut self, id: &TempSeq) {
        let _ = self.canceled.remove(id);
    }

    fn find(&self, id: &TempSeq) -> Option<DownloadSession> {
        self.running.get(id).cloned().or_else(|| self.canceled.get(id).map(|(session, _)| session.clone()))
    }

    fn session_count(&self) -> usize {
        self.running.len()
    }

    fn add(&mut self, session: DownloadSession) -> BuckyResult<DownloadSessionState> {
        if self.find(session.session_id()).is_some() {
            Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "duplicated"))
        } else {
            let state = session.state();
            match state {
                DownloadSessionState::Downloading => {
                    self.running.insert(session.session_id().clone(), session.clone());
                },
                _ => {
                    self.canceled.insert(session.session_id().clone(), (session.clone(), bucky_time_now()));
                }
            };
            Ok(state)
        }
    } 

    fn calc_speed(&mut self, when: Timestamp) -> u32 {
        self.cur_speed = self.speed_counter.update(when);
        if self.running.len() > 0 {
            self.history_speed.update(Some(self.cur_speed), when);
        } else {
            self.history_speed.update(None, when);
        }
        self.cur_speed
    }

    fn cur_speed(&self) -> u32 {
        self.cur_speed
    }
    
    fn history_speed(&self) -> u32 {
        self.history_speed.average()
    }

    fn on_time_escape(&mut self, now: Timestamp, msl: Duration) -> Vec<DownloadSession> {
        let mut to_remove = LinkedList::new();
        for (id, (_, when)) in self.canceled.iter() {
            if now > *when && (now - *when) > 2 * msl.as_micros() as u64 {
                to_remove.push_back(id.clone());
            }
        }

        for id in to_remove {
            self.canceled.remove(&id);
        }

        self.running.values().cloned().collect()
    }
}


struct UploadState {
    canceled: BTreeMap<TempSeq, (UploadSession, Timestamp)>, 
    cur_speed: u32, 
    history_speed: HistorySpeed, 
}


impl UploadState {
    fn new(history_speed: HistorySpeed) -> Self {
        Self {
            canceled: BTreeMap::new(), 
            cur_speed: 0, 
            history_speed
        }
    }

    fn cur_speed(&self) -> u32 {
        self.cur_speed
    }
    
    fn history_speed(&self) -> u32 {
        self.history_speed.average()
    }

    fn on_time_escape(&mut self, now: Timestamp, msl: Duration) {
        let mut to_remove = LinkedList::new();
        for (id, (_, when)) in self.canceled.iter() {
            if now > *when && (now - *when) > 2 * msl.as_micros() as u64 {
                to_remove.push_back(id.clone());
            }
        }

        for id in to_remove {
            self.canceled.remove(&id);
        }
    }
}

struct ChannelImpl {
    config: Config, 
    stack: WeakStack, 
    tunnel: TunnelGuard, 
    command_tunnel: DatagramTunnelGuard, 
    command_seq: TempSeqGenerator,  
    download_seq: TempSeqGenerator, 
    state: RwLock<StateImpl>, 
}

#[derive(Clone)]
pub struct Channel(Arc<ChannelImpl>);

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Channel{{local:{}, remote:{}}}", Stack::from(&self.0.stack).local_device_id(), self.tunnel().remote())
    }
}

#[derive(Clone)]
pub struct WeakChannel(Weak<ChannelImpl>);

impl WeakChannel {
    pub fn to_strong(&self) -> Option<Channel> {
        self.0.upgrade().map(|s| Channel(s))
    }
}

impl Channel {
    pub fn new(
        weak_stack: WeakStack, 
        tunnel: TunnelGuard, 
        command_tunnel: DatagramTunnelGuard
    ) -> Self {
        let stack = Stack::from(&weak_stack);
        let config = stack.config().ndn.channel.clone();
        Self(Arc::new(ChannelImpl {
            stack: weak_stack, 
            tunnel, 
            command_tunnel, 
            command_seq: TempSeqGenerator::new(), 
            download_seq: TempSeqGenerator::new(), 
            state: RwLock::new(StateImpl {
                upload: UploadState::new(HistorySpeed::new(0, config.history_speed.clone())), 
                download: DownloadState::new(HistorySpeed::new(0, config.history_speed.clone())), 
                tunnels: vec![]
            }), 
            config, 
        }))
    }

    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }

    pub fn to_weak(&self) -> WeakChannel {
        WeakChannel(Arc::downgrade(&self.0))
    }

    pub fn tunnel(&self) -> &TunnelGuard {
        &self.0.tunnel
    }

    pub fn config(&self) -> &Config {
        &self.0.config
    }

    fn default_tunnel(&self) -> BuckyResult<DynamicChannelTunnel> {
        self.tunnel_of(self.0.tunnel.default_tunnel()?)
    }

    fn tunnel_of(&self, raw_tunnel: DynamicTunnel) -> BuckyResult<DynamicChannelTunnel> {
        let mut state = self.0.state.write().unwrap();
        if let Some(exists) = state.tunnels.iter().find(|t| t.raw_ptr_eq(&raw_tunnel)) {
            Ok(exists.clone_as_tunnel())
        } else {
            let tunnel = new_channel_tunnel(self, raw_tunnel)?;
            state.tunnels.push(tunnel.clone_as_tunnel());
            Ok(tunnel)
        }
    }

    pub fn upload(
        &self,  
        chunk: ChunkId, 
        session_id: TempSeq, 
        piece_type: ChunkCodecDesc, 
        encoder: Box<dyn ChunkEncoder>
    ) -> BuckyResult<UploadSession> {
        let tunnel = self.default_tunnel()?;
        let session = UploadSession::new(chunk, session_id, piece_type, tunnel.upload_state(encoder), self.clone());
        tunnel.uploaders().add(session.clone());

        {
            let channel = self.clone();
            let session = session.clone();
            task::spawn(async move {
                let _ = session.wait_finish().await;
                let mut state = channel.0.state.write().unwrap();
                if state.tunnels.iter().find_map(|tunnel| tunnel.uploaders().remove(session.session_id())).is_some() {
                    state.upload.canceled.insert(session.session_id().clone(), (session, bucky_time_now()));
                }
            });
        }
        
        Ok(session)
    }

    pub fn download(
        &self,  
        chunk: ChunkId, 
        source: DownloadSource<DeviceId>, 
        cache: ChunkStreamCache, 
        referer: Option<String>, 
        group_path: Option<String>
    ) -> BuckyResult<DownloadSession> {
        let session = DownloadSession::interest(
            chunk, 
            self.gen_download_seq(), 
            self.clone(), 
	        source, 
            cache,
            referer, 
            group_path
        );

        let session_state = self.0.state.write().unwrap().download.add(session.clone()).map_err(|err| {
            debug!("{} add session {} failed for {}", self, session, err);
            err
        })?;

        match session_state {
            DownloadSessionState::Downloading => {
                {
                    let session = session.clone();
                    let channel = self.clone();
                    task::spawn(async move {
                        let _ = session.wait_finish().await;
                        channel.0.state.write().unwrap().download.cancel(session.session_id());
                    });
                }
                session.start();
            },
            _ => {
                // do nothing
            }
        };
        debug!("{} add session {}", self, session);
        Ok(session)
    } 

    pub(super) fn gen_command_seq(&self) -> TempSeq {
        self.0.command_seq.generate()
    }

    pub(super) fn gen_download_seq(&self) -> TempSeq {
        self.0.download_seq.generate()
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
            self.tunnel().remote(), 
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
            self.tunnel().remote(), 
            datagram::ReservedVPort::Channel as u16);
    }

    
    // 明文tunnel发送PieceControl
    pub(super) fn send_piece_control(&self, control: PieceControl) {
        if let Ok(tunnel) = self.tunnel().default_tunnel() {
            info!("{} will send piece control {:?}", self, control);
            let _ = control.split_send(&tunnel);
        } else {
            debug!("{} ignore send piece control {:?} for channel dead", self, control);
        }
    }

    pub(super) fn on_datagram(&self, datagram: Datagram) -> BuckyResult<()> {
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
        }
    }

    pub fn stack(&self) -> Stack {
        Stack::from(&self.0.stack)
    }

    pub fn state(&self) -> ChannelState {
        ChannelState::Active
    }

    pub fn calc_speed(&self, when: Timestamp) -> (u32, u32) {
        let mut state = self.0.state.write().unwrap();

        let mut upload_count = 0;
        let mut upload_speed = 0;
        for tunnel in &state.tunnels {
            let (speed, count) = tunnel.uploaders().calc_speed(when);
            upload_count += count;
            upload_speed += speed;
        }
        state.upload.cur_speed = upload_speed;

        if upload_count > 0 {
            state.upload.history_speed.update(Some(upload_speed), when);
        } else {
            state.upload.history_speed.update(None, when);
        }
        

        state.download.calc_speed(when); 

        (state.download.cur_speed(), state.upload.cur_speed())
    }

    pub fn download_session_count(&self) -> u32 {
        self.0.state.read().unwrap().download.session_count() as u32
    }

    pub fn download_cur_speed(&self) -> u32 {
        self.0.state.read().unwrap().download.cur_speed()
    }

    pub fn download_history_speed(&self) -> u32 {
        self.0.state.read().unwrap().download.history_speed()
    }

    pub fn upload_session_count(&self) -> u32 {
        self.0.state.read().unwrap().tunnels.iter().map(|tunnel| tunnel.uploaders().count()).sum::<usize>() as u32
    }

    pub fn upload_cur_speed(&self) -> u32 {
        self.0.state.read().unwrap().upload.cur_speed()
    }

    pub fn upload_history_speed(&self) -> u32 {
        self.0.state.read().unwrap().upload.history_speed()
    }

    async fn on_interest(&self, command: &Interest) -> BuckyResult<()> {
        info!("{} got interest {:?}", self, command);
        let session = {
            let state = self.0.state.write().unwrap();
            if let Some(session) = state.tunnels.iter().find_map(|tunnel| tunnel.uploaders().find(&command.session_id)) {
                Some(session)
            } else {
                state.upload.canceled.get(&command.session_id).map(|(session, _)| session.clone())
            }
        };
        
        
        if let Some(session) = session {
            info!("{} ignore {:?} for upload session exists", self, command);
            session.on_interest(self, command)
        } else {
            let stack = self.stack();
            stack.ndn().event_handler().on_newly_interest(&self.stack(), command, self).await
        }
    }

    fn on_resp_interest(&self, command: &RespInterest) -> BuckyResult<()> {
        if let Some(session) = self.0.state.read().unwrap().download.find(&command.session_id) {
            session.on_resp_interest(self, command)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "session not found"))
        }
    }
}


impl Channel {
    pub fn on_raw_data(&self, data: &[u8], tunnel: DynamicTunnel) -> BuckyResult<()> {
        let tunnel = self.tunnel_of(tunnel)?;
        let (cmd_code, buf) = u8::raw_decode(data)?;
        let cmd_code = PackageCmdCode::try_from(cmd_code)?;
        match cmd_code {
            PackageCmdCode::PieceData => {
                let piece = PieceData::decode_from_raw_data(buf)?;
                let _ = tunnel.on_piece_data(&piece)?;
                self.on_piece_data(piece, &tunnel)
            }, 
            PackageCmdCode::PieceControl => {
                let (ctrl, _) = PieceControl::raw_decode(buf)?;
                self.on_piece_control(&ctrl) 
            },
            PackageCmdCode::ChannelEstimate => {
                let (est, _) = ChannelEstimate::raw_decode(buf)?;
                tunnel.on_resp_estimate(&est) 
            }
            _ => unreachable!()
        }
    }

    fn on_piece_data(&self, piece: PieceData, tunnel: &DynamicChannelTunnel) -> BuckyResult<()> {
        trace!("{} got piece data est_seq:{:?} chunk:{} desc:{:?} data:{}", self, piece.est_seq, piece.chunk, piece.desc, piece.data.len());

        let session = {
            let mut state = self.0.state.write().unwrap();
            state.download.speed_counter.on_recv(piece.data.len());
            state.download.find(&piece.session_id)
        };

        if let Some(session) = session {
            session.push_piece_data(self, &piece, tunnel);
        } else {
            let strong_stack = Stack::from(&self.0.stack);
            // 这里可能要保证同步到同线程处理,重入会比较麻烦
            match strong_stack.ndn().event_handler().on_unknown_piece_data(&self.stack(), &piece, self) {
                Ok(_session) => {
                    //FIXME： 如果新建了任务，这里应当继续接受piece data
                },
                Err(_err) => {
                    // 通过新建一个canceled的session来回复piece control
                    // let session = DownloadSession::canceled(
                    //     self.0.stack.clone(), 
                    //     piece.chunk.clone(), 
                    //     piece.session_id.clone(), 
                    //     self.clone(),
                    //     err);
                    // let _ = self.0.downloaders.add(session.clone());
                    // session.push_piece_data(&piece);
                }  
            }
        }

        Ok(())
    }

    fn on_piece_control(&self, ctrl: &PieceControl) -> BuckyResult<()> {
        debug!("{} got piece control {:?}", self, ctrl);
        
        let session = {
            let state = self.0.state.write().unwrap();
            if let Some(session) = state.tunnels.iter().find_map(|tunnel| tunnel.uploaders().find(&ctrl.session_id)) {
                Some(session)
            } else {
                state.upload.canceled.get(&ctrl.session_id).map(|(session, _)| session.clone())
            }
        };

        if let Some(session) = session {
            session.on_piece_control(self, ctrl)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "session not found"))
        }
    }

    pub fn on_time_escape(&self, now: Timestamp) {
        struct Elements {
            tunnels: Vec<DynamicChannelTunnel>, 
            downloaders: Vec<DownloadSession>, 
        }

        let elements = {
            let mut state = self.0.state.write().unwrap();
            let mut tunnels = state.tunnels.iter().filter_map(|t| {
                if TunnelState::Dead != t.state() {
                    Some(t.clone_as_tunnel())
                } else {
                    None
                }
            }).collect();
            std::mem::swap(&mut tunnels, &mut state.tunnels);
            let downloaders = state.download.on_time_escape(now, self.config().msl);
            state.upload.on_time_escape(now, self.config().msl);
            Elements {
                tunnels, 
                downloaders
            }
        };

        for tunnel in elements.tunnels {
            let _ = tunnel.on_time_escape(now);
        }

        for session in elements.downloaders {
            let _ = session.on_time_escape(now);
        }
    }
}


