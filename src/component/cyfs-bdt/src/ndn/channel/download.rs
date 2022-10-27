use log::*;
use std::{
    time::Duration, 
    sync::{RwLock}, 
};
use async_std::{
    sync::Arc, 
};
use futures::future::AbortRegistration;
use cyfs_base::*;
use crate::{
    types::*, 
};
use super::super::{
    chunk::*, 
    types::*
};
use super::{
    protocol::v0::*,
    channel::Channel, 
};


struct InterestingState {
    waiters: StateWaiter,  
    last_send_time: Timestamp, 
    next_send_time: Timestamp,  
    history_speed: HistorySpeed, 
    cache: ChunkStreamCache,
}

struct DownloadingState {
    waiters: StateWaiter, 
    last_pushed: Timestamp, 
    decoder: Box<dyn ChunkDecoder>, 
    speed_counter: SpeedCounter, 
    history_speed: HistorySpeed, 
}


struct FinishedState {
    send_ctrl_time: Timestamp, 
}

struct CanceledState {
    send_ctrl_time: Timestamp, 
    err: BuckyError
}

pub enum DownloadSessionState {
    Downloading(u32), 
    Finished,
    Canceled(BuckyErrorCode),
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
            Self::Interesting(_) => DownloadSessionState::Downloading(0), 
            Self::Downloading(_) => DownloadSessionState::Downloading(0), 
            Self::Finished(_) => DownloadSessionState::Finished, 
            Self::Canceled(canceled) => DownloadSessionState::Canceled(canceled.err.code()),
        }
    }
}


struct SessionImpl {
    chunk: ChunkId, 
    channel: Channel, 
    session_id: TempSeq, 
    desc: ChunkEncodeDesc, 
    referer: Option<String>, 
    state: RwLock<StateImpl>, 
}

#[derive(Clone)]
pub struct DownloadSession(Arc<SessionImpl>);

impl std::fmt::Display for DownloadSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DownloadSession{{session_id:{:?}, chunk:{}, remote:{}}}", self.session_id(), self.chunk(), self.channel().remote())
    }
}


impl DownloadSession {
    pub fn new(
        chunk: ChunkId, 
        session_id: TempSeq, 
        channel: Channel, 
	    referer: Option<String>, 
        desc: ChunkEncodeDesc,  
        cache: ChunkStreamCache,
    ) -> Self {
        let now = bucky_time_now(); 
        let session = Self(Arc::new(SessionImpl {
            chunk, 
            session_id,  
            desc,
	        referer, 
            state: RwLock::new(StateImpl::Interesting(InterestingState { 
                history_speed: HistorySpeed::new(
                    channel.initial_download_session_speed(), 
                    channel.config().history_speed.clone()), 
                waiters: StateWaiter::new(), 
                last_send_time: now, 
                next_send_time: now + channel.config().resend_interval.as_micros() as u64,
                cache
            })), 
            channel, 
        }));

        let interest = Interest {
            session_id: session.session_id().clone(), 
            chunk: session.chunk().clone(), 
            prefer_type: session.desc().clone(), 
            referer: session.referer().cloned(), 
            from: None
        };
        info!("{} sent {:?}", session, interest);
        session.channel().interest(interest);

        session
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn desc(&self) -> &ChunkEncodeDesc {
        &self.0.desc
    }

    pub fn referer(&self) -> Option<&String> {
        self.0.referer.as_ref()
    }

    pub fn channel(&self) -> &Channel {
        &self.0.channel
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
                StateImpl::Canceled(canceled) => NextStep::Return(DownloadSessionState::Canceled(canceled.err.code())),
            }
        };
        match next_step {
            NextStep::Wait(waker) => StateWaiter::wait(waker, || self.state()).await,
            NextStep::Return(state) => state
        }
    }

    pub(super) fn push_piece_data(&self, piece: &PieceData) {
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
                    if finished.send_ctrl_time < now 
                        && Duration::from_micros(now - finished.send_ctrl_time) > self.channel().config().resend_interval {
                        finished.send_ctrl_time = now;
                        RespControl(PieceControlCommand::Finish)
                    } else {
                        Ignore
                    }
                }, 
                Canceled(canceled) => {
                    let now = bucky_time_now();
                    if canceled.send_ctrl_time < now 
                        && Duration::from_micros(now - canceled.send_ctrl_time) > self.channel().config().resend_interval {
                        canceled.send_ctrl_time = now;
                        RespControl(PieceControlCommand::Cancel)
                    } else {
                        Ignore
                    }
                }, 
            }
        };

        let resp_control = |command: PieceControlCommand| {
            self.channel().send_piece_control(PieceControl {
                sequence: self.channel().gen_command_seq(), 
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
                            downloading.last_pushed = bucky_time_now();
                        }
                        if result.finished {
                            let mut waiters = StateWaiter::new();
                            std::mem::swap(&mut waiters, &mut downloading.waiters);
                            info!("{} finished", self);
                            *state = Finished(FinishedState {
                                send_ctrl_time: bucky_time_now(), 
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
                            let decoder = StreamDecoder::new(self.chunk(), self.desc(), interesting.cache.clone());
                            let mut downloading = DownloadingState {
                                history_speed: interesting.history_speed.clone(), 
                                speed_counter: SpeedCounter::new(piece.data.len()), 
                                decoder: decoder.clone_as_decoder(), 
                                waiters: StateWaiter::new(), 
                                last_pushed: 0, 
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

    pub(super) fn on_resp_interest(&self, resp_interest: &RespInterest) -> BuckyResult<()> {
        match &resp_interest.err {
            BuckyErrorCode::Ok => unimplemented!(), 
            BuckyErrorCode::WouldBlock => {
                use StateImpl::*;
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    Interesting(interesting) => {
                        interesting.next_send_time = bucky_time_now() + self.channel().config().block_interval.as_micros() as u64;  
                    }, 
                    Downloading(downloading) => {
                        downloading.last_pushed = bucky_time_now();
                    },
                    _ => {}
                }   
            }, 
            _ => {
                self.cancel_by_error(BuckyError::new(resp_interest.err, "remote resp interest error"));
            }
        }
        Ok(())
    }

    fn resend_interest(&self) -> BuckyResult<()> {
        let interest = Interest {
            session_id: self.session_id().clone(), 
            chunk: self.chunk().clone(), 
            prefer_type: self.desc().clone(), 
            from: None,
            referer: self.referer().cloned()
        };
        info!("{} sent {:?}", self, interest);
        self.channel().interest(interest);
        Ok(())
    }


    pub fn cancel_by_error(&self, err: BuckyError) {
        error!("{} cancel by err {}", self, err);

        let mut waiters = StateWaiter::new();
        {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Interesting(interesting) => {
                    std::mem::swap(&mut waiters, &mut interesting.waiters);
                    *state = StateImpl::Canceled(CanceledState {
                        send_ctrl_time: 0, 
                        err
                    });
                },
                StateImpl::Downloading(downloading) => {
                    std::mem::swap(&mut waiters, &mut downloading.waiters);
                    *state = StateImpl::Canceled(CanceledState {
                        send_ctrl_time: 0, 
                        err
                    });
                },
	    	    StateImpl::Finished(_) => {
                    *state = StateImpl::Canceled(CanceledState {
                        send_ctrl_time: 0, 
                        err
                    });
                },
                _ => {}
            };
        }
        waiters.wake();
    }

    pub(super) fn on_time_escape(&self, now: Timestamp) -> BuckyResult<()> {
        enum NextStep {
            None, 
            SendInterest, 
            SendPieceControl(PieceControl), 
            Cancel, 
        }

        let next_step = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Interesting(interesting) => {
                    if now > interesting.next_send_time {
                        interesting.next_send_time = now + 2 * (interesting.next_send_time - interesting.last_send_time);
                        interesting.last_send_time = now;
                        NextStep::SendInterest
                    } else {
                        NextStep::None
                    }
                }, 
                StateImpl::Downloading(downloading) => {
                    if now > downloading.last_pushed 
                        && Duration::from_micros(now - downloading.last_pushed) > self.channel().config().resend_interval {
                        if let Some((max_index, lost_index)) = downloading.decoder.require_index() {
                            downloading.last_pushed = now;
                            {
                                debug!("{} dectect loss piece max_index:{:?} lost_index:{:?}", self, max_index, lost_index);
                                NextStep::SendPieceControl(PieceControl {
                                    sequence: self.channel().gen_command_seq(), 
                                    session_id: self.session_id().clone(), 
                                    chunk: downloading.decoder.chunk().clone(), 
                                    command: PieceControlCommand::Continue, 
                                    max_index, 
                                    lost_index
                                })
                            }
                        } else {
                            NextStep::None
                        }
                    } else {
                        NextStep::None
                    }
                },
                StateImpl::Finished(_) => NextStep::None, 
                StateImpl::Canceled(_) => NextStep::None,
            }
        };
        
        match next_step {
            NextStep::None => Ok(()), 
            NextStep::Cancel => {
                self.cancel_by_error(BuckyError::new(BuckyErrorCode::Timeout, "interest timeout"));
                Err(BuckyError::new(BuckyErrorCode::Timeout, "interest timeout"))
            }, 
            NextStep::SendInterest => {
                let _ = self.resend_interest();
                Ok(())
            }, 
            NextStep::SendPieceControl(ctrl) => {
                self.channel().send_piece_control(ctrl);
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




