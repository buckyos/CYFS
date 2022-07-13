use log::*;
use std::{
    time::Duration, 
    sync::{RwLock, atomic::{AtomicU32, AtomicU64, Ordering}}
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
    scheduler::*, 
    chunk::*,
};
use super::{
    types::*, 
    protocol::*, 
    provider::*,
    channel::Channel, 
};


struct InterestingState {
    waiters: StateWaiter, 
    start_send_time: Timestamp, 
    last_send_time: Timestamp
}

struct DownloadingState {
    waiters: StateWaiter, 
    session_type: SessionType, 
}

enum SessionType {
    Stream(Box<dyn DownloadSessionProvider>), 
    Raptor(Box<dyn DownloadSessionProvider>), 
}

impl SessionType {
    fn provider(&self) -> &Box<dyn DownloadSessionProvider> {
        match self {
            Self::Stream(provider) => provider,
            Self::Raptor(provider) => provider
        }
    }
}

struct FinishedState {
    send_ctrl_time: Timestamp, 
    chunk: Option<Arc<Vec<u8>>>
}

struct CanceledState {
    send_ctrl_time: Timestamp, 
    err: BuckyError
}

struct RedirectState {
    send_ctrl_time: Timestamp,
    redirect: DeviceId,
    redirect_referer: String,
}

enum StateImpl {
    Init(StateWaiter), 
    Interesting(InterestingState), 
    Downloading(DownloadingState),
    Finished(FinishedState), 
    Canceled(CanceledState),
    Redirect(RedirectState),
} 

impl StateImpl {
    fn to_task_state(&self) -> TaskState {
        match self {
            Self::Init(_) => TaskState::Running(0), 
            Self::Interesting(_) => TaskState::Running(0), 
            Self::Downloading(_) => TaskState::Running(0), 
            Self::Finished(_) => TaskState::Finished, 
            Self::Canceled(canceled) => TaskState::Canceled(canceled.err.code()),
            Self::Redirect(redirect) => TaskState::Redirect(redirect.redirect.clone(), redirect.redirect_referer.clone()),
        }
    }
}

struct SessionPPSImpl {
    t: u64,
    pps_mini: u32,
    check_time: AtomicU64,
    pps: AtomicU32,
}

#[derive(Clone)]
pub struct SessionPPS(Arc<SessionPPSImpl>);

impl SessionPPS {
    pub fn default() -> Self {
        let pps = 3;
        let pps_def_t = 10;
        let pps_def_pps_mini = pps_def_t*pps;

        Self(Arc::new(SessionPPSImpl {
            t: std::time::Duration::from_secs(pps_def_t).as_micros() as u64,
            pps_mini: pps_def_pps_mini as u32,
            check_time: AtomicU64::new(0),
            pps: AtomicU32::new(0),
        }))
    }

    pub fn new(t: u64, pps_mini: u32) -> Self {
        Self(Arc::new(SessionPPSImpl {
            t: std::time::Duration::from_secs(t).as_micros() as u64,
            pps_mini,
            check_time: AtomicU64::new(0),
            pps: AtomicU32::new(0),
        }))
    }

    pub fn new_package(&self) {
        self.0.pps.fetch_add(1, Ordering::SeqCst);
    }

    pub fn check(&self) -> bool {
        let mut less_than_pps_mini = false;
        let now = bucky_time_now();
        let check_time = self.0.check_time.load(Ordering::SeqCst);
        if now > check_time {
            if check_time > 0 {
                let pps = self.0.pps.load(Ordering::SeqCst);
                less_than_pps_mini = pps < self.0.pps_mini;
            }
            self.0.check_time.store(now + self.0.t, Ordering::SeqCst);
            self.0.pps.store(0, Ordering::SeqCst);
        }

        less_than_pps_mini
    }
}

struct SessionImpl {
    chunk: ChunkId, 
    session_id: TempSeq, 
    channel: Channel, 
    state: RwLock<StateImpl>, 
    prefer_type: PieceSessionType, 
    view: Option<ChunkView>,
    referer: Option<String>,
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
        prefer_type: PieceSessionType,
        view: ChunkView,
	    referer: Option<String>) -> Self {
        Self(Arc::new(SessionImpl {
            chunk, 
            session_id, 
            channel, 
            prefer_type, 
	        referer, 
            state: RwLock::new(StateImpl::Init(StateWaiter::new())),
            view: Some(view),
        }))
    }

    pub fn canceled(
        chunk: ChunkId, 
        session_id: TempSeq, 
        channel: Channel, 
        err: BuckyError
    ) -> Self {
        Self(Arc::new(SessionImpl {
            chunk, 
            session_id, 
            channel, 
            prefer_type: PieceSessionType::Unknown, 
            referer: None, 
            state: RwLock::new(StateImpl::Canceled(CanceledState {
                send_ctrl_time: 0, 
                err
            })),
            view: None,
        }))
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn prefer_type(&self) -> &PieceSessionType {
        &self.0.prefer_type
    }

    pub fn referer(&self) -> Option<&String> {
        self.0.referer.as_ref()
    }

    pub fn channel(&self) -> &Channel {
        &self.0.channel
    }  

    pub fn task_state(&self) -> TaskState {
        (&self.0.state.read().unwrap()).to_task_state()
    }

    pub fn session_id(&self) -> &TempSeq {
        &self.0.session_id
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn start(&self) -> BuckyResult<()> {
        self.channel().clear_dead();

        info!("{} try start", self);
        {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Init(waiters) => {
                    let seq = self.channel().gen_command_seq();
                    let now = bucky_time_now();
                    let mut interesting = InterestingState {
                        waiters: StateWaiter::new(), 
                        start_send_time: now, 
                        last_send_time: now, 
                    };
                    std::mem::swap(&mut interesting.waiters, waiters);
                    *state = StateImpl::Interesting(interesting);
                    Ok(seq)
                }, 
                _ => {
                    let err = BuckyError::new(BuckyErrorCode::ErrorState, "not in init state");
                    error!("{} try start failed for {}", self, err);
                    Err(err)
                }
            }
        }?;

        let interest = Interest {
            session_id: self.session_id().clone(), 
            chunk: self.chunk().clone(), 
            prefer_type: self.prefer_type().clone(), 
            referer: self.referer().cloned()
        };
        info!("{} sent {:?}", self, interest);
        self.channel().interest(interest);
        Ok(())
    }

    pub async fn wait_finish(&self) -> TaskState {
        enum NextStep {
            Wait(AbortRegistration), 
            Return(TaskState)
        }
        let next_step = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Init(waiters) => NextStep::Wait(waiters.new_waiter()), 
                StateImpl::Interesting(interesting) => NextStep::Wait(interesting.waiters.new_waiter()), 
                StateImpl::Downloading(downloading) => NextStep::Wait(downloading.waiters.new_waiter()),
                StateImpl::Finished(_) => NextStep::Return(TaskState::Finished), 
                StateImpl::Canceled(canceled) => NextStep::Return(TaskState::Canceled(canceled.err.code())),
                StateImpl::Redirect(cn) => NextStep::Return(TaskState::Redirect(cn.redirect.clone(), cn.redirect_referer.clone())),
            }
        };
        match next_step {
            NextStep::Wait(waker) => StateWaiter::wait(waker, || self.task_state()).await,
            NextStep::Return(state) => state
        }
    }
    
    pub fn take_chunk_content(&self) -> Option<Arc<Vec<u8>>> {
        let state = &mut *self.0.state.write().unwrap();
        match state {
            StateImpl::Finished(finished) => {
                if finished.chunk.is_some() {
                    let mut chunk = None;
                    std::mem::swap(&mut chunk, &mut finished.chunk);
                    info!("{} chunk content taken", self);
                    chunk
                } else {
                    None
                }
            }, 
            _ => None
        }
    }

    pub(super) fn push_piece_data(&self, piece: &PieceData) {
        enum NextStep {
            EnterDownloading, 
            RespControl(PieceControlCommand), 
            Ignore, 
            Push(Box<dyn DownloadSessionProvider>)
        }
        use NextStep::*;
        use StateImpl::*;
        let next_step = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                Interesting(_) => {
                    EnterDownloading
                }, 
                Downloading(downloading) => {
                    Push(downloading.session_type.provider().clone_as_provider())
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
                Init(_) | _ => {
                    unreachable!()
                }
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
            })
        };

        let push_to_decoder = |provider: Box<dyn DownloadSessionProvider>| {
            if provider.push_piece_data(piece).unwrap() {
                if let Some(waiters) = {
                    let state = &mut *self.0.state.write().unwrap();
                    match state {
                        Downloading(downloading) => {
                            let mut waiters = StateWaiter::new();
                            std::mem::swap(&mut waiters, &mut downloading.waiters);
                            info!("{} finished", self);
                            *state = Finished(FinishedState {
                                send_ctrl_time: bucky_time_now(), 
                                chunk: Some(downloading.session_type.provider().decoder().chunk_content().unwrap())
                            });
                            Some(waiters)
                        }, 
                        _ => None
                    }
                } {
                    waiters.wake();
                    resp_control(PieceControlCommand::Finish)
                }
            }    
        };

        match next_step {
            EnterDownloading => {
                match *self.prefer_type() {
			//TODO: 其他session type支持
                    PieceSessionType::Stream(_) => {
                        let provider = StreamDownload::new(
                            self.chunk(), 
                            self.session_id().clone(), 
                            self.channel().clone());

                        if let Some(provider) = {
                            let state = &mut *self.0.state.write().unwrap();
                            match state {
                                Interesting(interesting) => {
                                    let mut downloading = DownloadingState {
                                        session_type: SessionType::Stream(provider.clone_as_provider()),
                                        waiters: StateWaiter::new(), 
                                    };
                                    std::mem::swap(&mut downloading.waiters, &mut interesting.waiters);
                                    *state = Downloading(downloading);
                                    Some(provider.clone_as_provider())
                                }, 
                                Downloading(downloading) => {
                                    Some(downloading.session_type.provider().clone_as_provider())
                                }, 
                                _ => None
                            }
                        } {
                            push_to_decoder(provider);
                        }
                    },
                    PieceSessionType::RaptorA(_) | PieceSessionType::RaptorB(_)  => {
                        let view = self.0.view.as_ref().unwrap();
                        let decoder = view.raptor_decoder();
                        let provider = RaptorDownload::new(decoder);

                        if let Some(provider) = {
                            let state = &mut *self.0.state.write().unwrap();
                            match state {
                                Interesting(interesting) => {
                                    let mut downloading = DownloadingState {
                                        session_type: SessionType::Raptor(provider.clone_as_provider()),
                                        waiters: StateWaiter::new(), 
                                    };
                                    std::mem::swap(&mut downloading.waiters, &mut interesting.waiters);
                                    *state = Downloading(downloading);
                                    Some(provider.clone_as_provider())
                                }, 
                                Downloading(downloading) => {
                                    Some(downloading.session_type.provider().clone_as_provider())
                                }, 
                                _ => None
                            }
                        } {
                            push_to_decoder(provider);
                        }
                    },
                    _ => {
                    }
                };
            }, 
            Push(s) => {
                push_to_decoder(s)
            }, 
            RespControl(cmd) => resp_control(cmd), 
            Ignore => {}
        }
    }

    pub(super) fn on_resp_interest(&self, resp_interest: &RespInterest) -> BuckyResult<()> {
        match &resp_interest.err {
            BuckyErrorCode::Ok => unimplemented!(),
            BuckyErrorCode::SessionRedirect => {
                if resp_interest.redirect.is_some() && resp_interest.redirect_referer.is_some() {
                    let redirect_node = resp_interest.redirect.as_ref().unwrap();
                    let referer = resp_interest.redirect_referer.as_ref().unwrap();
                    self.redirect_interest(redirect_node, referer);
                } else {
                    self.cancel_by_error(BuckyError::new(resp_interest.err, "need redirect, but has not new node"));
                }
            },
            _ => {
                self.cancel_by_error(BuckyError::new(resp_interest.err, "remote resp interest error"));
            }
        }
        Ok(())
    }

    fn resend_interest(&self) -> BuckyResult<()> {
        {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Interesting(interesting) => {
                    interesting.last_send_time = bucky_time_now(); 
                    Ok(())
                }, 
                _ => Err(BuckyError::new(BuckyErrorCode::ErrorState, "not in interesting state"))
            }
        }?;
        let interest = Interest {
            session_id: self.session_id().clone(), 
            chunk: self.chunk().clone(), 
            prefer_type: self.prefer_type().clone(), 
            referer: self.referer().cloned()
        };
        info!("{} sent {:?}", self, interest);
        self.channel().interest(interest);
        Ok(())
    }

    pub fn redirect_interest(&self, redirect_node: &DeviceId, referer: &String) {
        info!("{} redirect to {} refer {}", self, redirect_node, referer);

        let mut waiters = StateWaiter::new();
        {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Init(_waiters) => {
                    std::mem::swap(&mut waiters, _waiters);
                    *state = StateImpl::Redirect(RedirectState {send_ctrl_time: 0, 
                                                                redirect: redirect_node.clone(),
                                                                redirect_referer: referer.clone()});
                },
                StateImpl::Interesting(interesting) => {
                    std::mem::swap(&mut waiters, &mut interesting.waiters);
                    *state = StateImpl::Redirect(RedirectState {send_ctrl_time: 0, 
                                                                redirect: redirect_node.clone(),
                                                                redirect_referer: referer.clone()});
                },
                StateImpl::Downloading(downloading) => {
                    std::mem::swap(&mut waiters, &mut downloading.waiters);
                    *state = StateImpl::Redirect(RedirectState {send_ctrl_time: 0, 
                                                                redirect: redirect_node.clone(),
                                                                redirect_referer: referer.clone()});
                },
	    	    StateImpl::Finished(_) => {
                    *state = StateImpl::Redirect(RedirectState {send_ctrl_time: 0, 
                                                                redirect: redirect_node.clone(),
                                                                redirect_referer: referer.clone()});
                },
                _ => {}
            }
        }
        waiters.wake();
    }

    pub fn cancel_by_error(&self, err: BuckyError) {
        error!("{} cancel by err {}", self, err);

        let mut waiters = StateWaiter::new();
        {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Init(_waiters) => {
                    std::mem::swap(&mut waiters, _waiters);
                    *state = StateImpl::Canceled(CanceledState {
                        send_ctrl_time: 0, 
                        err
                    });
                },
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
            Cancel, 
            CallProvider(Box<dyn DownloadSessionProvider>),
        }
        let next_step = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StateImpl::Init(_) => NextStep::None, 
                StateImpl::Interesting(interesting) => {
                    if now > interesting.start_send_time
                        && Duration::from_micros(now - interesting.start_send_time) > self.channel().config().resend_timeout {
                        NextStep::Cancel
                    } else if now > interesting.last_send_time 
                        && Duration::from_micros(now - interesting.last_send_time) > self.channel().config().resend_interval {
                        NextStep::SendInterest
                    } else {
                        NextStep::None
                    }
                }, 
                StateImpl::Downloading(downloading) => {
                    NextStep::CallProvider(downloading.session_type.provider().clone_as_provider())
                },
                StateImpl::Finished(_) => NextStep::None, 
                StateImpl::Canceled(_) => NextStep::None,
                StateImpl::Redirect(_) => NextStep::None,
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
            NextStep::CallProvider(provider) => {
                match self.0.prefer_type {
                    PieceSessionType::RaptorA(_) => {
                    },
                    _ => {}
                }

                match provider.on_time_escape(now) {
                    Ok(_) => {
                        Ok(())
                    },
                    Err(err) => {
                        self.cancel_by_error(err);
                        Err(BuckyError::new(BuckyErrorCode::Timeout, "session timeout"))
                    }
                }
            }
        }
    }
}

