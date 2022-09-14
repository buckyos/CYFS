use log::*;
use std::{
    time::Duration, 
    sync::{RwLock, atomic::{AtomicU64, Ordering}}
};
use async_std::{
    sync::Arc, 
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::{
    chunk::*, 
    download::*
};
use super::{
    types::*, 
    protocol::v0::*, 
    provider::*, 
    channel::Channel, 
};

struct UploadingState {
    speed_counter: SpeedCounter,  
    history_speed: HistorySpeed, 
    pending_from: Timestamp, 
    provider: Box<dyn UploadSessionProvider>
}

enum StateImpl {
    Init, 
    Uploading(UploadingState),
    Finished, 
    Canceled(BuckyErrorCode),
}

pub enum UploadSessionState {
    Uploading(u32), 
    Finished,
    Canceled(BuckyErrorCode),
}

impl StateImpl {
    fn to_session_state(&self) -> UploadSessionState {
        match self {
            StateImpl::Init => UploadSessionState::Uploading(0), 
            StateImpl::Uploading(_) => UploadSessionState::Uploading(0), 
            StateImpl::Finished => UploadSessionState::Finished, 
            StateImpl::Canceled(err) => UploadSessionState::Canceled(*err),
        }
    }
}
struct SessionImpl {
    chunk: ChunkId, 
    session_id: TempSeq, 
    piece_type: PieceSessionType, 
    channel: Channel, 
    state: RwLock<StateImpl>, 
    last_active: AtomicU64, 
}



#[derive(Clone)]
pub struct UploadSession(Arc<SessionImpl>);

impl std::fmt::Display for UploadSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UploadSession{{session_id:{:?}, chunk:{}, remote:{}}}", self.session_id(), self.chunk(), self.channel().remote())
    }
}

impl UploadSession {
    pub fn new(
        chunk: ChunkId, 
        session_id: TempSeq, 
        piece_type: PieceSessionType, 
        channel: Channel
    ) -> Self {
        Self(Arc::new(SessionImpl {
            chunk, 
            session_id, 
            piece_type, 
            channel, 
            state: RwLock::new(StateImpl::Init), 
            last_active: AtomicU64::new(0), 
        }))
    }

    pub fn state(&self) -> UploadSessionState {
        self.0.state.read().unwrap().to_session_state()
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn piece_type(&self) -> &PieceSessionType {
        &self.0.piece_type
    }

    pub fn channel(&self) -> &Channel {
        &self.0.channel
    }

    pub fn session_id(&self) -> &TempSeq {
        &self.0.session_id
    }

    pub fn start(&self, chunk_encoder: TypedChunkEncoder) {
        info!("{} started", self);
        let state = &mut *self.0.state.write().unwrap();
        match state {
            StateImpl::Init => {
                *state = match *self.piece_type() {
                    PieceSessionType::Stream(..) => {
                        let encoder = match chunk_encoder {
                            TypedChunkEncoder::Range(encoder) => encoder,
                            _ => unreachable!()
                        };
                        StateImpl::Uploading(
                            UploadingState {
                                pending_from: 0, 
                                history_speed: HistorySpeed::new(0, self.channel().config().history_speed.clone()), 
                                speed_counter: SpeedCounter::new(0), 
                                provider: StreamUpload::new(
                                    self.session_id().clone(), 
                                    encoder).clone_as_provider()
                            })
                    },
                    _ => {
                        let encoder = match chunk_encoder {
                            TypedChunkEncoder::Range(encoder) => encoder,
                            _ => unreachable!()
                        };
                        StateImpl::Uploading(
                            UploadingState {
                                pending_from: 0, 
                                history_speed: HistorySpeed::new(0, self.channel().config().history_speed.clone()), 
                                speed_counter: SpeedCounter::new(0), 
                                provider: StreamUpload::new(
                                    self.session_id().clone(), 
                                    encoder).clone_as_provider()
                            })
                    }
                };
            }, 
            _ => unreachable!()
        }
    }

    pub(super) fn next_piece(&self, buf: &mut [u8]) -> BuckyResult<usize> {
        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StateImpl::Uploading(uploading) => {
                    Some(uploading.provider.clone_as_provider())
                }, 
                _ => None
            }
        };
        if let Some(provider) = provider {
            match provider.next_piece(buf) {
                Ok(len) => {
                    let state = &mut *self.0.state.write().unwrap();
                    match state {
                        StateImpl::Uploading(uploading) => {
                            if len > 0 {
                                uploading.speed_counter.on_recv(len);
                                uploading.pending_from = 0;
                            } else {
                                match provider.state() {
                                    ChunkEncoderState::Ready => {
                                        uploading.pending_from = bucky_time_now()
                                    }, 
                                    _ => {
                                        uploading.pending_from = 0;
                                    }
                                };
                            }
                            Ok(len)
                        },
                        _ => {
                            Err(BuckyError::new(BuckyErrorCode::ErrorState, "not uploading"))
                        }
                    }
                   
                }, 
                Err(err) => {
                    self.cancel_by_error(BuckyError::new(err.code(), "encoder failed"));
                    Err(err)
                }
            }
        } else {
            Ok(0)
        }
    }

    pub(super) fn cancel_by_error(&self, err: BuckyError) {
        let state = &mut *self.0.state.write().unwrap();
        match state {
            StateImpl::Canceled(_) => {}, 
            _ => {
                info!("{} canceled by err:{}", self, err);
                *state = StateImpl::Canceled(err.code());
            }
        }
    }

    // 把第一个包加到重发队列里去
    pub fn on_interest(&self, interest: &Interest) -> BuckyResult<()> {
        enum NextStep {
            CallProvider(Box<dyn UploadSessionProvider>), 
            RespInterest(BuckyErrorCode), 
            None
        }
        self.0.last_active.store(bucky_time_now(), Ordering::SeqCst);
        let next_step = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StateImpl::Uploading(uploading) => {
                    NextStep::CallProvider(uploading.provider.clone_as_provider())
                }, 
                StateImpl::Canceled(err) => {
                    NextStep::RespInterest(*err)
                }, 
                _ => {
                    NextStep::None
                }
            }
        };

        match next_step {
            NextStep::CallProvider(provider) => provider.on_interest(interest), 
            NextStep::RespInterest(err) => {
                let resp_interest = RespInterest {
                    session_id: self.session_id().clone(), 
                    chunk: self.chunk().clone(), 
                    err, 
                    redirect: None,
                    redirect_referer: None,
                    to: None,
                };
                self.channel().resp_interest(resp_interest);
                Ok(())
            }, 
            NextStep::None => Ok(())
        }
    }

    pub(super) fn on_piece_control(&self, ctrl: &PieceControl) -> BuckyResult<()> {
        self.0.last_active.store(bucky_time_now(), Ordering::SeqCst);
        enum NextStep {
            CallProvider(Box<dyn UploadSessionProvider>), 
            RespInterest(BuckyErrorCode), 
            None
        }

        let next_step = match ctrl.command {
            PieceControlCommand::Finish => {
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    StateImpl::Uploading(_) => {
                        info!("{} finished", self);
                        *state = StateImpl::Finished;
                    }, 
                    _ => {

                    }
                }
                NextStep::None
            }, 
            PieceControlCommand::Cancel => {
                *self.0.state.write().unwrap() = StateImpl::Canceled(BuckyErrorCode::Interrupted);
                info!("{} canceled by remote", self);
                NextStep::None
            }, 
            PieceControlCommand::Continue => {
                let state = &*self.0.state.read().unwrap();
                match state {
                    StateImpl::Uploading(uploading) => NextStep::CallProvider(uploading.provider.clone_as_provider()),
                    StateImpl::Canceled(err) => NextStep::RespInterest(*err),  
                    _ => NextStep::None
                }
            },
            _ => unimplemented!()
        };

        match next_step {
            NextStep::CallProvider(provider) => provider.on_piece_control(ctrl), 
            NextStep::RespInterest(err) => {
                let resp_interest = RespInterest {
                    session_id: self.session_id().clone(), 
                    chunk: self.chunk().clone(), 
                    err: err,
                    redirect: None,
                    redirect_referer: None,
                    to: None,
                };
                self.channel().resp_interest(resp_interest);
                Ok(())
            }, 
            NextStep::None => {
                Ok(())
            }
        }
    }

    pub(super) fn on_time_escape(&self, now: Timestamp) -> Option<UploadSessionState> {
        let state = &mut *self.0.state.write().unwrap();
        match state {
            StateImpl::Init => Some(UploadSessionState::Uploading(0)), 
            StateImpl::Uploading(uploading) => {
                if uploading.pending_from > 0 
                    && now > uploading.pending_from 
                    && Duration::from_micros(now - uploading.pending_from) > self.channel().config().resend_timeout {
                    error!("{} canceled for pending timeout", self);
                    *state = StateImpl::Canceled(BuckyErrorCode::Timeout);
                    Some(UploadSessionState::Canceled(BuckyErrorCode::Timeout))
                } else {
                    Some(UploadSessionState::Uploading(0))
                }
            }, 
            StateImpl::Finished => None,
            StateImpl::Canceled(err) => {
                let last_active = self.0.last_active.load(Ordering::SeqCst);
                if now > last_active 
                    && Duration::from_micros(now - last_active) > 2 * self.channel().config().msl {
                    None
                } else {
                    Some(UploadSessionState::Canceled(*err))
                }
            },
        }
    }
}

