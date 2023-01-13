use log::*;
use std::{
    sync::{RwLock}, 
};
use async_std::{
    sync::Arc, 
};
use futures::future::AbortRegistration;
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::{
    chunk::*, 
    types::*, 
    download::*
};
use super::{
    protocol::v0::*,
    channel::{Channel, WeakChannel},
    tunnel::{
        DynamicChannelTunnel, 
        TunnelDownloadState
    } 
};


struct InterestingState {
    waiters: StateWaiter,  
    last_send_time: Option<Timestamp>, 
    next_send_time: Option<Timestamp>,  
    history_speed: HistorySpeed, 
    cache: ChunkStreamCache, 
    channel: Channel
}

struct DownloadingState {
    waiters: StateWaiter, 
    tunnel_state: Box<dyn TunnelDownloadState>, 
    decoder: Box<dyn ChunkDecoder>, 
    speed_counter: SpeedCounter, 
    history_speed: HistorySpeed, 
    channel: Channel
}


struct FinishedState {
    send_ctrl_time: Option<(WeakChannel, Timestamp)>, 
}

struct CanceledState {
    send_ctrl_time: Option<(WeakChannel, Timestamp)>, 
    err: BuckyError
}

#[derive(Debug, Clone)]
pub enum DownloadSessionState {
    Downloading, 
    Finished,
    Canceled(BuckyError),
}

enum StateImpl {
    Interesting(InterestingState), 
    Downloading(DownloadingState),
    Finished(FinishedState), 
    Canceled(CanceledState),
} 

impl StateImpl {
    fn to_session_state(&self) -> DownloadSessionState {
        match self {
            Self::Interesting(_) => DownloadSessionState::Downloading, 
            Self::Downloading(_) => DownloadSessionState::Downloading, 
            Self::Finished(_) => DownloadSessionState::Finished, 
            Self::Canceled(canceled) => DownloadSessionState::Canceled(canceled.err.clone()),
        }
    }
}


struct SessionImpl {
    chunk: ChunkId, 
    session_id: TempSeq, 
    source: DownloadSource<DeviceId>, 
    referer: Option<String>,  
    group_path: Option<String>, 
    state: RwLock<StateImpl>, 
}

#[derive(Clone)]
pub struct DownloadSession(Arc<SessionImpl>);

impl std::fmt::Display for DownloadSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DownloadSession{{session_id:{:?}, chunk:{}, source:{}}}", self.session_id(), self.chunk(), self.source().target)
    }
}


impl DownloadSession {
    pub fn error(
        chunk: ChunkId, 
        session_id: Option<TempSeq>, 
        source: DownloadSource<DeviceId>, 
        referer: Option<String>, 
        group_path: Option<String>,
        err: BuckyError
    ) -> Self {
        Self(Arc::new(SessionImpl {
            chunk, 
            session_id: session_id.unwrap_or_default(), 
            source, 
            referer, 
            group_path, 
            state: RwLock::new(StateImpl::Canceled(CanceledState {
                send_ctrl_time: None, 
                err
            })), 
        }))
    }

    pub fn interest(
        chunk: ChunkId, 
        session_id: TempSeq, 
        channel: Channel, 
        source: DownloadSource<DeviceId>, 
        cache: ChunkStreamCache,
        referer: Option<String>, 
        group_path: Option<String>
    ) -> Self { 
        Self(Arc::new(SessionImpl {
            chunk, 
            session_id,  
            source, 
            referer, 
            group_path, 
            state: RwLock::new(StateImpl::Interesting(InterestingState { 
                history_speed: HistorySpeed::new(0, channel.config().history_speed.clone()), 
                waiters: StateWaiter::new(), 
                last_send_time: None, 
                next_send_time: None, 
                channel, 
                cache
            })), 
        }))
    }

    pub fn source(&self) -> &DownloadSource<DeviceId> {
        &self.0.source
    }

    pub fn referer(&self) -> &Option<String> {
        &self.0.referer
    }

    pub fn group_path(&self) -> &Option<String> {
        &self.0.group_path
    }

    pub fn start(&self) {
        let send = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Interesting(interesting) => {
                    if interesting.last_send_time.is_none() {
                        let now = bucky_time_now();
                        interesting.last_send_time = Some(now);
                        interesting.next_send_time = Some(now + interesting.channel.config().resend_interval.as_micros() as u64);
                        Some(interesting.channel.clone())
                    } else {
                        None
                    }
                }, 
                _ => None
            }
        };
        
        if let Some(channel) = send {
            let interest = Interest {
                session_id: self.session_id().clone(), 
                chunk: self.chunk().clone(), 
                prefer_type: self.source().codec_desc.clone(), 
                referer: self.referer().clone(), 
                group_path: self.group_path().clone(), 
                from: None, 
            };
            info!("{} sent {:?}", self, interest);
            channel.interest(interest);
        }
       
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn state(&self) -> DownloadSessionState {
        (&self.0.state.read().unwrap()).to_session_state()
    }

    pub fn session_id(&self) -> &TempSeq {
        &self.0.session_id
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub async fn wait_finish(&self) -> DownloadSessionState {
        enum NextStep {
            Wait(AbortRegistration), 
            Return(DownloadSessionState)
        }
        let next_step = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Interesting(interesting) => NextStep::Wait(interesting.waiters.new_waiter()), 
                StateImpl::Downloading(downloading) => NextStep::Wait(downloading.waiters.new_waiter()),
                StateImpl::Finished(_) => NextStep::Return(DownloadSessionState::Finished), 
                StateImpl::Canceled(canceled) => NextStep::Return(DownloadSessionState::Canceled(canceled.err.clone())),
            }
        };
        match next_step {
            NextStep::Wait(waker) => StateWaiter::wait(waker, || self.state()).await,
            NextStep::Return(state) => state
        }
    }

    pub(super) fn push_piece_data(&self, channel: &Channel, piece: &PieceData, tunnel: &DynamicChannelTunnel) {
        enum NextStep {
            EnterDownloading, 
            RespControl(PieceControlCommand), 
            Ignore, 
            Push(Box<dyn ChunkDecoder>)
        }
        use NextStep::*;
        use StateImpl::*;
        let next_step = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                Interesting(_) => EnterDownloading, 
                Downloading(downloading) => {
                    downloading.speed_counter.on_recv(piece.data.len());
                    Push(downloading.decoder.clone_as_decoder())
                },
                Finished(finished) => {
                    let now = bucky_time_now();
                    if finished.send_ctrl_time.is_none() {
                        finished.send_ctrl_time = Some((channel.to_weak(), now + channel.config().resend_interval.as_micros() as u64))
                    } {
                        Ignore
                    }
                }, 
                Canceled(canceled) => {
                    let now = bucky_time_now();
                    if canceled.send_ctrl_time.is_none() {
                        canceled.send_ctrl_time = Some((channel.to_weak(), now + channel.config().resend_interval.as_micros() as u64))
                    } {
                        Ignore
                    }
                }, 
            }
        };

        let resp_control = |command: PieceControlCommand| {
            channel.send_piece_control(PieceControl {
                sequence: channel.gen_command_seq(), 
                session_id: self.session_id().clone(), 
                chunk: self.chunk().clone(), 
                command, 
                max_index: None, 
                lost_index: None
            });
        };

        let push_to_decoder = |provider: Box<dyn ChunkDecoder>| {
            let result = provider.push_piece_data(piece).unwrap(); 
            if let Some(waiters) = {
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    Downloading(downloading) => {
                        if result.valid {
                            downloading.tunnel_state.as_mut().on_piece_data();
                        }
                        if result.finished {
                            let mut waiters = StateWaiter::new();
                            std::mem::swap(&mut waiters, &mut downloading.waiters);
                            info!("{} finished", self);
                            *state = Finished(FinishedState {
                                send_ctrl_time: None, 
                            });
                            Some(waiters)
                        } else {
                            None
                        } 
                    }, 
                    _ => None
                }
            } {
                waiters.wake();
                resp_control(PieceControlCommand::Finish);
            }
        };

        match next_step {
            EnterDownloading => {
                if let Some(decoder) = {
                    let state = &mut *self.0.state.write().unwrap();
                    match state {
                        Interesting(interesting) => {
                            let decoder = StreamDecoder::new(self.chunk(), &self.source().codec_desc, interesting.cache.clone());
                            let mut downloading = DownloadingState {
                                channel: channel.clone(), 
                                tunnel_state: tunnel.as_ref().download_state(), 
                                history_speed: interesting.history_speed.clone(), 
                                speed_counter: SpeedCounter::new(piece.data.len()), 
                                decoder: decoder.clone_as_decoder(), 
                                waiters: StateWaiter::new(), 
                            };
                            std::mem::swap(&mut downloading.waiters, &mut interesting.waiters);
                            *state = Downloading(downloading);
                            Some(decoder.clone_as_decoder())
                        }, 
                        Downloading(downloading) => {
                            Some(downloading.decoder.clone_as_decoder())
                        }, 
                        _ => None
                    }
                } {
                    push_to_decoder(decoder)
                }
                
            }, 
            Push(decoder) => {
                push_to_decoder(decoder)
            }, 
            RespControl(cmd) => resp_control(cmd), 
            Ignore => {}
        }
    }

    pub(super) fn on_resp_interest(&self, channel: &Channel, resp_interest: &RespInterest) -> BuckyResult<()> {
        match resp_interest.err {
            BuckyErrorCode::Ok => unimplemented!(), 
            BuckyErrorCode::WouldBlock => {
                use StateImpl::*;
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    Interesting(interesting) => {
                        interesting.next_send_time = Some(bucky_time_now() + channel.config().block_interval.as_micros() as u64);  
                    }, 
                    Downloading(downloading) => {
                        downloading.tunnel_state.on_resp_interest();
                    }, 
                    Finished(finished) => {
                        let now = bucky_time_now();
                        if finished.send_ctrl_time.is_none() {
                            finished.send_ctrl_time = Some((channel.to_weak(), now + channel.config().resend_interval.as_micros() as u64))
                        } 
                    }, 
                    Canceled(canceled) => {
                        let now = bucky_time_now();
                        if canceled.send_ctrl_time.is_none() {
                            canceled.send_ctrl_time = Some((channel.to_weak(), now + channel.config().resend_interval.as_micros() as u64))
                        } 
                    }
                }   
            }, 
            _ => {
                error!("{} cancel by err {}", self, resp_interest.err);

                let mut waiters = StateWaiter::new();
                {
                    let state = &mut *self.0.state.write().unwrap();
                    match state {
                        StateImpl::Interesting(interesting) => {
                            std::mem::swap(&mut waiters, &mut interesting.waiters);
                            *state = StateImpl::Canceled(CanceledState {
                                send_ctrl_time: None, 
                                err: BuckyError::new(resp_interest.err, "cancel by remote")
                            });
                        },
                        StateImpl::Downloading(downloading) => {
                            std::mem::swap(&mut waiters, &mut downloading.waiters);
                            *state = StateImpl::Canceled(CanceledState {
                                send_ctrl_time: None, 
                                err: BuckyError::new(resp_interest.err, "cancel by remote")
                            });
                        },
                        _ => {}
                    };
                }
                
                waiters.wake();
            }
        }
        Ok(())
    }

    fn resend_interest(&self, channel: &Channel) -> BuckyResult<()> {
        let interest = Interest {
            session_id: self.session_id().clone(), 
            chunk: self.chunk().clone(), 
            prefer_type: self.source().codec_desc.clone(), 
            referer: self.referer().clone(), 
            from: None, 
            group_path: None
        };
        info!("{} sent {:?}", self, interest);
        channel.interest(interest);
        Ok(())
    }


    pub fn cancel_by_error(&self, err: BuckyError) {
        error!("{} cancel by err {}", self, err);

        let mut waiters = StateWaiter::new();
        let send = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Interesting(interesting) => {
                    std::mem::swap(&mut waiters, &mut interesting.waiters);
                    let channel = interesting.channel.clone();
                    *state = StateImpl::Canceled(CanceledState {
                        send_ctrl_time: None, 
                        err
                    });
                    Some(channel)
                },
                StateImpl::Downloading(downloading) => {
                    std::mem::swap(&mut waiters, &mut downloading.waiters);
                    let channel = downloading.channel.clone();
                    *state = StateImpl::Canceled(CanceledState {
                        send_ctrl_time: None, 
                        err
                    });
                    Some(channel)
                },
                _ => None
            }
        };
        waiters.wake();

        if let Some(channel) = send {
            channel.send_piece_control(PieceControl {
                sequence: channel.gen_command_seq(), 
                session_id: self.session_id().clone(), 
                chunk: self.chunk().clone(), 
                command: PieceControlCommand::Cancel, 
                max_index: None, 
                lost_index: None
            });
        }
    }

    pub(super) fn on_time_escape(&self, now: Timestamp) -> BuckyResult<()> {
        enum NextStep {
            None, 
            SendInterest(Channel), 
            SendPieceControl(Channel, PieceControl), 
        }

        let next_step = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Interesting(interesting) => {
                    if let Some(next_send_time) = interesting.next_send_time {
                        if now > next_send_time {
                            interesting.next_send_time = Some(now + 2 * (next_send_time - interesting.last_send_time.unwrap()));
                            interesting.last_send_time = Some(now);
                            NextStep::SendInterest(interesting.channel.clone())
                        } else {
                            NextStep::None
                        }
                    } else {
                        NextStep::None
                    }
                   
                }, 
                StateImpl::Downloading(downloading) => {
                    if downloading.tunnel_state.as_mut().on_time_escape(now) {
                        if let Some((max_index, lost_index)) = downloading.decoder.require_index() {
                            debug!("{} dectect loss piece max_index:{:?} lost_index:{:?}", self, max_index, lost_index);
                            NextStep::SendPieceControl(downloading.channel.clone(), PieceControl {
                                sequence: downloading.channel.gen_command_seq(), 
                                session_id: self.session_id().clone(), 
                                chunk: self.chunk().clone(), 
                                command: PieceControlCommand::Continue, 
                                max_index, 
                                lost_index
                            })
                        } else {
                            NextStep::None
                        }
                    } else {
                        NextStep::None
                    }
                },
                StateImpl::Finished(finished) => {
                    if let Some((channel, send_time)) = &finished.send_ctrl_time {
                        if now > *send_time {
                            let channel = channel.to_strong();
                            finished.send_ctrl_time = None;
                            if let Some(channel) = channel {
                                let ctrl = PieceControl {
                                    sequence: channel.gen_command_seq(), 
                                    session_id: self.session_id().clone(), 
                                    chunk: self.chunk().clone(), 
                                    command: PieceControlCommand::Finish, 
                                    max_index: None, 
                                    lost_index: None
                                }; 
                                
                                NextStep::SendPieceControl(channel, ctrl) 
                            } else {
                                NextStep::None
                            }
                        } else {
                            NextStep::None
                        }
                    } else {
                        NextStep::None
                    }
                }
                StateImpl::Canceled(canceled) => {
                    if let Some((channel, send_time)) = &canceled.send_ctrl_time {
                        if now > *send_time {
                            let channel = channel.to_strong();
                            canceled.send_ctrl_time = None;
                            if let Some(channel) = channel {
                                let ctrl = PieceControl {
                                    sequence: channel.gen_command_seq(), 
                                    session_id: self.session_id().clone(), 
                                    chunk: self.chunk().clone(), 
                                    command: PieceControlCommand::Cancel, 
                                    max_index: None, 
                                    lost_index: None
                                };
                                NextStep::SendPieceControl(channel, ctrl) 
                            } else {
                                NextStep::None
                            }
                        } else {
                            NextStep::None
                        }
                    } else {
                        NextStep::None
                    }
                }
            }
        };
        
        match next_step {
            NextStep::None => Ok(()), 
            NextStep::SendInterest(channel) => {
                let _ = self.resend_interest(&channel);
                Ok(())
            }, 
            NextStep::SendPieceControl(channel, ctrl) => {
                channel.send_piece_control(ctrl);
                Ok(())
            }
        }
    }

    pub fn calc_speed(&self, when: Timestamp) -> u32 {
        let state = &mut *self.0.state.write().unwrap();
        match state {
            StateImpl::Interesting(interesting) => {
                interesting.history_speed.update(Some(0), when);
                0
            },
            StateImpl::Downloading(downloading) => {
                let cur_speed = downloading.speed_counter.update(when);
                downloading.history_speed.update(Some(cur_speed), when);
                cur_speed
            },
            _ => 0
        }
    }

    pub fn cur_speed(&self) -> u32 {
        let state = &*self.0.state.read().unwrap();
        match state {
            StateImpl::Downloading(downloading) => downloading.history_speed.latest(),
            _ => 0
        }
    }

    pub fn history_speed(&self) -> u32 {
        let state = &*self.0.state.read().unwrap();
        match state {
            StateImpl::Interesting(interesting) => interesting.history_speed.average(),
            StateImpl::Downloading(downloading) => downloading.history_speed.average(),
            _ => 0
        }
    }
}




