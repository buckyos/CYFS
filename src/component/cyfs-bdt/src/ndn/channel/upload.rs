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
    scheduler::*,
    chunk::*, 
};
use super::{
    types::*, 
    protocol::*, 
    provider::*, 
    channel::Channel, 
};

struct UploadingState {
    provider: Box<dyn UploadSessionProvider>
}

enum StateImpl {
    Init, 
    Uploading(UploadingState),
    Finished, 
    Canceled(BuckyErrorCode),
    Redirect(DeviceId, String /* redirect_referer */),
    WaitRedirect,
}

impl StateImpl {
    fn to_task_state(&self) -> TaskState {
        match self {
            StateImpl::Init => TaskState::Pending, 
            StateImpl::Uploading(_) => TaskState::Running(0), 
            StateImpl::Finished => TaskState::Finished, 
            StateImpl::Canceled(err) => TaskState::Canceled(*err),
            StateImpl::Redirect(_, _) => TaskState::Finished,
            StateImpl::WaitRedirect => TaskState::WaitRedirect,
        }
    }
}
struct SessionImpl {
    chunk: ChunkId, 
    session_id: TempSeq, 
    piece_type: PieceSessionType, 
    channel: Channel, 
    resource: ResourceManager, 
    resource_quota: ResourceQuota,
    state: RwLock<StateImpl>, 
    last_active: AtomicU64, 
    pending_from: AtomicU64
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
        channel: Channel, 
        quota: ResourceQuota,
        owner: ResourceManager) -> Self {
        Self(Arc::new(SessionImpl {
            chunk, 
            session_id, 
            piece_type, 
            channel, 
            resource: ResourceManager::new(Some(owner)), 
            resource_quota: quota,
            state: RwLock::new(StateImpl::Init), 
            last_active: AtomicU64::new(0), 
            pending_from: AtomicU64::new(0)
        }))
    }

    pub fn canceled(
        chunk: ChunkId, 
        session_id: TempSeq, 
        piece_type: PieceSessionType, 
        channel: Channel, 
        err: BuckyErrorCode
    ) -> Self {
        Self(Arc::new(SessionImpl {
            chunk, 
            session_id, 
            piece_type, 
            channel, 
            resource: ResourceManager::new(None), 
            resource_quota: ResourceQuota::new(),
            state: RwLock::new(StateImpl::Canceled(err)), 
            last_active: AtomicU64::new(0), 
            pending_from: AtomicU64::new(0)
        }))
    }

    pub fn redirect(chunk: ChunkId, 
                    session_id: TempSeq, 
                    piece_type: PieceSessionType, 
                    channel: Channel,
                    dump_pn: DeviceId, 
                    referer: String) -> Self {
        Self(Arc::new(SessionImpl {
            chunk,
            session_id, 
            piece_type, 
            channel, 
            resource: ResourceManager::new(None), 
	    resource_quota: ResourceQuota::new(),
            state: RwLock::new(StateImpl::Redirect(dump_pn, referer)),
            last_active: AtomicU64::new(0), 
            pending_from: AtomicU64::new(0),
        }))
    }

    pub fn wait_redirect(chunk: ChunkId, 
                         session_id: TempSeq,
                         piece_type: PieceSessionType, 
                         channel: Channel) -> Self {
        Self(Arc::new(SessionImpl {
            chunk,
            session_id, 
            piece_type, 
            channel, 
            resource: ResourceManager::new(None), 
	    resource_quota: ResourceQuota::new(),
            state: RwLock::new(StateImpl::WaitRedirect),
            last_active: AtomicU64::new(0), 
            pending_from: AtomicU64::new(0),
        }))
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
                    PieceSessionType::Stream(_) => {
                        let encoder = match chunk_encoder {
                            TypedChunkEncoder::Range(encoder) => encoder,
                            _ => unreachable!()
                        };
                        StateImpl::Uploading(
                            UploadingState {
                                provider: StreamUpload::new(
                                    self.session_id().clone(), 
                                    encoder).clone_as_provider()
                            })
                    },
                    PieceSessionType::RaptorA(_) => {
                        let encoder = match chunk_encoder {
                            TypedChunkEncoder::Raptor(encoder) => encoder,
                            _ => unreachable!()
                        };
                        StateImpl::Uploading(
                            UploadingState {
                                provider: RaptorUpload::new(
                                    self.session_id().clone(), 
                                    encoder,
                                    0,
                                false).clone_as_provider()
                            })
                    },
                    PieceSessionType::RaptorB(_) => {
                        let encoder = match chunk_encoder {
                            TypedChunkEncoder::Raptor(encoder) => encoder,
                            _ => unreachable!()
                        };
                        StateImpl::Uploading(
                            UploadingState {
                                provider: RaptorUpload::new(
                                    self.session_id().clone(), 
                                    encoder,
                                    std::u16::MAX,
                            true).clone_as_provider()
                            })
                    },
                    _ => {
                        let encoder = match chunk_encoder {
                            TypedChunkEncoder::Range(encoder) => encoder,
                            _ => unreachable!()
                        };
                        StateImpl::Uploading(
                            UploadingState {
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
                    if len > 0 {
                        self.0.pending_from.store(0, Ordering::SeqCst);
                    } else {
                        let now = match provider.state() {
                            ChunkEncoderState::Ready => bucky_time_now(),
                            _ => {0}
                        };
                        let _ = self.0.pending_from.compare_exchange(0, now, Ordering::AcqRel, Ordering::Acquire);
                    }
                    Ok(len)
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
            RedirectInterest(DeviceId, String),
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
                StateImpl::Redirect(cache_node, referer) => {
                    NextStep::RedirectInterest(cache_node.clone(), referer.clone())
                },
                StateImpl::WaitRedirect => {
                    NextStep::RespInterest(BuckyErrorCode::SessionWaitRedirect)
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
                };
                self.channel().resp_interest(resp_interest);
                Ok(())
            }, 
            NextStep::RedirectInterest(cache_node, referer) => {
                let resp_interest = RespInterest {
                    session_id: self.session_id().clone(), 
                    chunk: self.chunk().clone(), 
                    err: BuckyErrorCode::SessionRedirect,
                    redirect: Some(cache_node),
                    redirect_referer: Some(referer),
                };
                self.channel().resp_interest(resp_interest);
                Ok(())
            }
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
                };
                self.channel().resp_interest(resp_interest);
                Ok(())
            }, 
            NextStep::None => {
                let _ = self.0.resource_quota.remove_child(self.channel().remote());
                Ok(())
            }
        }
    }

    pub(super) fn on_time_escape(&self, now: Timestamp) -> Option<TaskState> {
        let state = &mut *self.0.state.write().unwrap();
        match state {
            StateImpl::Init => Some(TaskState::Running(0)), 
            StateImpl::Uploading(_) => {
                
                let pending_from = self.0.pending_from.load(Ordering::SeqCst);
                if pending_from > 0 
                    && now > pending_from 
                    && Duration::from_micros(now - pending_from) > self.channel().config().resend_timeout {
                    error!("{} canceled for pending timeout", self);
                    *state = StateImpl::Canceled(BuckyErrorCode::Timeout);
                    Some(TaskState::Canceled(BuckyErrorCode::Timeout))
                } else {
                    Some(TaskState::Running(0))
                }
            }, 
            StateImpl::Finished => None,
            StateImpl::Canceled(err) => {
                let last_active = self.0.last_active.load(Ordering::SeqCst);
                if now > last_active 
                    && Duration::from_micros(now - last_active) > 2 * self.channel().config().msl {
                    None
                } else {
                    Some(TaskState::Canceled(*err))
                }
            }
            StateImpl::Redirect(_cache_node, _referer) => None,
            StateImpl::WaitRedirect => None,
        }
    }
}

impl TaskSchedule for UploadSession {
    fn schedule_state(&self) -> TaskState {
        self.0.state.read().unwrap().to_task_state()
    }

    fn resource(&self) -> &ResourceManager {
        &self.0.resource
    }

    fn start(&self) -> TaskState {
        self.0.state.read().unwrap().to_task_state()
    }
}