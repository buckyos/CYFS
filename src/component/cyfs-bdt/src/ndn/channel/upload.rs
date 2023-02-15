use log::*;
use std::{
    ops::Range, 
    sync::{RwLock}
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
    upload::*,
    types::*
};
use super::{ 
    protocol::v0::*, 
    channel::Channel, 
};

struct UploadingState {
    channel: Channel, 
    waiters: StateWaiter, 
    speed_counter: SpeedCounter,  
    uploaded: u64, 
    history_speed: HistorySpeed, 
    encoder: Box<dyn ChunkEncoder>
}

struct StateImpl {
    task_state: TaskStateImpl, 
    control_state: NdnTaskControlState, 
}

enum TaskStateImpl {
    Uploading(UploadingState),
    Finished(u64), 
    Error(BuckyError),
}

struct SessionImpl {
    remote: DeviceId, 
    chunk: ChunkId, 
    session_id: TempSeq, 
    piece_type: ChunkCodecDesc, 
    state: RwLock<StateImpl>, 
}

#[derive(Clone)]
pub struct UploadSession(Arc<SessionImpl>);

impl std::fmt::Display for UploadSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UploadSession{{session_id:{:?}, chunk:{}, remote:{}}}", self.session_id(), self.chunk(), self.remote())
    }
}

impl UploadSession {
    pub fn new(
        chunk: ChunkId, 
        session_id: TempSeq, 
        piece_type: ChunkCodecDesc, 
        encoder: Box<dyn ChunkEncoder>, 
        channel: Channel
    ) -> Self {
        Self(Arc::new(SessionImpl {
            remote: channel.tunnel().remote().clone(), 
            chunk, 
            session_id, 
            piece_type, 
            state: RwLock::new(StateImpl{
                task_state: TaskStateImpl::Uploading(UploadingState {
                    waiters: StateWaiter::new(), 
                    history_speed: HistorySpeed::new(0, channel.config().history_speed.clone()), 
                    speed_counter: SpeedCounter::new(0), 
                    uploaded: 0, 
                    encoder, 
                    channel
                }),
                control_state: NdnTaskControlState::Normal
            }), 
        }))
    }

    pub fn remote(&self) -> &DeviceId {
        &self.0.remote
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn piece_type(&self) -> &ChunkCodecDesc {
        &self.0.piece_type
    }

    pub fn session_id(&self) -> &TempSeq {
        &self.0.session_id
    }


    pub(super) fn next_piece(&self, buf: &mut [u8]) -> BuckyResult<usize> {
        let encoder = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Uploading(uploading) => {
                    Some(uploading.encoder.clone_as_encoder())
                }, 
                _ => None
            }
        };
        if let Some(encoder) = encoder {
            match encoder.next_piece(self.session_id(), buf) {
                Ok(len) => {
                    let mut state = self.0.state.write().unwrap();
                    match &mut state.task_state {
                        TaskStateImpl::Uploading(uploading) => {
                            if len > 0 {
                                uploading.speed_counter.on_recv(len);
                                uploading.uploaded += len as u64;
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
        let send = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.task_state {
                TaskStateImpl::Error(_) => None, 
                TaskStateImpl::Finished(_) => None,
                TaskStateImpl::Uploading(uploading) => {
                    let mut waiters = StateWaiter::new();
                    uploading.waiters.transfer_into(&mut waiters);
                    let channel = uploading.channel.clone();
                    info!("{} canceled by err:{}", self, err);
                    state.task_state = TaskStateImpl::Error(err.clone());
                    Some((waiters, channel))
                }
            }
        };

        if let Some((waiters, channel)) = send {
            let resp_interest = RespInterest {
                session_id: self.session_id().clone(), 
                chunk: self.chunk().clone(), 
                err: err.code(), 
                redirect: None,
                redirect_referer: None,
                to: None,
            };
            channel.resp_interest(resp_interest);

            waiters.wake();
        }
    }

    // 把第一个包加到重发队列里去
    pub fn on_interest(&self, channel: &Channel, _interest: &Interest) -> BuckyResult<()> {
        enum NextStep {
            ResetEncoder(Box<dyn ChunkEncoder>), 
            RespInterest(BuckyErrorCode), 
            None
        }
        let next_step = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Uploading(uploading) => {
                    NextStep::ResetEncoder(uploading.encoder.clone_as_encoder())
                }, 
                TaskStateImpl::Error(err) => {
                    NextStep::RespInterest(err.code())
                }, 
                _ => {
                    NextStep::None
                }
            }
        };

        match next_step {
            NextStep::ResetEncoder(encoder) => {
                debug!("{} will reset index", self);
                if !encoder.reset() {
                    let resp_interest = RespInterest {
                        session_id: self.session_id().clone(), 
                        chunk: self.chunk().clone(), 
                        err: BuckyErrorCode::WouldBlock, 
                        redirect: None,
                        redirect_referer: None,
                        to: None,
                    };
                    channel.resp_interest(resp_interest);
                }
                Ok(())
            }, 
            NextStep::RespInterest(err) => {
                let resp_interest = RespInterest {
                    session_id: self.session_id().clone(), 
                    chunk: self.chunk().clone(), 
                    err, 
                    redirect: None,
                    redirect_referer: None,
                    to: None,
                };
                channel.resp_interest(resp_interest);
                Ok(())
            }, 
            NextStep::None => Ok(())
        }
    }

    pub(super) fn on_piece_control(&self, channel: &Channel, ctrl: &PieceControl) -> BuckyResult<()> {
        enum NextStep {
            MergeIndex(Box<dyn ChunkEncoder>, u32, Vec<Range<u32>>), 
            RespInterest(BuckyErrorCode), 
            Notify(StateWaiter), 
            None
        }

        let next_step = match ctrl.command {
            PieceControlCommand::Finish => {
                let mut state = self.0.state.write().unwrap();
                match &mut state.task_state {
                    TaskStateImpl::Uploading(uploading) => {
                        info!("{} finished", self);
                        let mut waiters = StateWaiter::new();
                        uploading.waiters.transfer_into(&mut waiters); 
                        state.task_state = TaskStateImpl::Finished(uploading.uploaded);
                        NextStep::Notify(waiters)
                    }, 
                    _ => {
                        NextStep::None
                    }
                }
            }, 
            PieceControlCommand::Cancel => {
                info!("{} canceled by remote", self);
                let mut state = self.0.state.write().unwrap();
                match &mut state.task_state {
                    TaskStateImpl::Uploading(uploading) => {
                        info!("{} finished", self);
                        let mut waiters = StateWaiter::new();
                        uploading.waiters.transfer_into(&mut waiters); 
                        state.task_state = TaskStateImpl::Error(BuckyError::new(BuckyErrorCode::Interrupted, "cancel by remote"));
                        NextStep::Notify(waiters)
                    }, 
                    _ => {
                        NextStep::None
                    }
                }
            }, 
            PieceControlCommand::Continue => {
                let state = self.0.state.read().unwrap();
                match &state.task_state {
                    TaskStateImpl::Uploading(uploading) => {
                        if let Some(max_index) = ctrl.max_index {
                            NextStep::MergeIndex(uploading.encoder.clone_as_encoder(), max_index, ctrl.lost_index.clone().unwrap_or_default())
                        } else {
                            NextStep::None
                        }
                    },
                    TaskStateImpl::Error(err) => NextStep::RespInterest(err.code()),  
                    _ => NextStep::None
                }
            },
            _ => unimplemented!()
        };

        match next_step {
            NextStep::MergeIndex(encoder, max_index, lost_index) => {
                if !encoder.merge(max_index, lost_index) {
                    let resp_interest = RespInterest {
                        session_id: self.session_id().clone(), 
                        chunk: self.chunk().clone(), 
                        err: BuckyErrorCode::WouldBlock, 
                        redirect: None,
                        redirect_referer: None,
                        to: None,
                    };
                    channel.resp_interest(resp_interest);
                }
            }, 
            NextStep::RespInterest(err) => {
                let resp_interest = RespInterest {
                    session_id: self.session_id().clone(), 
                    chunk: self.chunk().clone(), 
                    err: err,
                    redirect: None,
                    redirect_referer: None,
                    to: None,
                };
                channel.resp_interest(resp_interest);
            }, 
            NextStep::Notify(waiters) => {
                waiters.wake();
            }, 
            NextStep::None => {
            }
        }
        Ok(())
    }

    pub async fn wait_finish(&self) -> NdnTaskState {
        let waiter = match &mut self.0.state.write().unwrap().task_state {
            TaskStateImpl::Uploading(uploading) => Some(uploading.waiters.new_waiter()), 
            _ => None, 
        };
        
        if let Some(waiter) = waiter {
            StateWaiter::wait(waiter, || self.state()).await
        } else {
            self.state()
        }
    }
}


impl NdnTask for UploadSession {
    fn clone_as_task(&self) -> Box<dyn NdnTask> {
        Box::new(self.clone())
    }
    
    fn state(&self) -> NdnTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Uploading(_) => NdnTaskState::Running,
            TaskStateImpl::Finished(_) => NdnTaskState::Finished, 
            TaskStateImpl::Error(err) => NdnTaskState::Error(err.clone()),
        }
    }

    fn control_state(&self) -> NdnTaskControlState {
        self.0.state.read().unwrap().control_state.clone()
    }

    fn transfered(&self) -> u64 {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Uploading(uploading) => uploading.uploaded,
            TaskStateImpl::Finished(uploaded) => *uploaded, 
            TaskStateImpl::Error(_) => 0,
        }
    }

    fn cur_speed(&self) -> u32 {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Uploading(uploading) => {
                uploading.history_speed.latest()
            }, 
            _ => 0
        }
    }

    fn history_speed(&self) -> u32 {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Uploading(uploading) => {
                uploading.history_speed.average()
            }, 
            _ => 0
        }
    }
}

#[async_trait::async_trait]
impl UploadTask for UploadSession {
    fn clone_as_upload_task(&self) -> Box<dyn UploadTask> {
        Box::new(self.clone())
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        match &mut self.0.state.write().unwrap().task_state {
            TaskStateImpl::Uploading(uploading) => {
                let cur_speed = uploading.speed_counter.update(when);
                uploading.history_speed.update(Some(cur_speed), when);
                cur_speed
            }, 
            _ => 0
        }
    }
}

