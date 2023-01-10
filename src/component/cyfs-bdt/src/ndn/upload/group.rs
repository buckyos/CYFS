use std::{
    collections::{HashMap}, 
    sync::{Arc, RwLock}
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::{
    types::*
};
use super::{
    common::*
};

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

struct UploadingState {
    entries: HashMap<String, Box<dyn UploadTask>>, 
    running: Vec<Box<dyn UploadTask>>, 
    closed: bool, 
    history_speed: HistorySpeed, 
}


enum TaskStateImpl {
    Uploading(UploadingState), 
    Finished, 
    Error(BuckyError), 
}

struct StateImpl {
    task_state: TaskStateImpl, 
    control_state: ControlStateImpl
}

struct TaskImpl {
    history_speed: HistorySpeedConfig, 
    state: RwLock<StateImpl>
}

#[derive(Clone)]
pub struct UploadGroup(Arc<TaskImpl>);

impl UploadGroup {
    pub fn new(history_speed: HistorySpeedConfig) -> Self {
        Self(Arc::new(TaskImpl {
            history_speed: history_speed.clone(), 
            state: RwLock::new(StateImpl { 
                task_state: TaskStateImpl::Uploading(UploadingState {
                    entries: Default::default(), 
                    running: Default::default(), 
                    closed: false, 
                    history_speed: HistorySpeed::new(0, history_speed), 
                }), 
                control_state: ControlStateImpl::Normal(StateWaiter::new())
            })
        }))
    }

    pub fn history_config(&self) -> &HistorySpeedConfig {
        &self.0.history_speed
    }
}

impl NdnTask for UploadGroup {
    fn clone_as_task(&self) -> Box<dyn NdnTask> {
        Box::new(self.clone())
    }
    
    fn state(&self) -> NdnTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Uploading(_) => NdnTaskState::Running, 
            TaskStateImpl::Finished => NdnTaskState::Finished, 
            TaskStateImpl::Error(err) => NdnTaskState::Error(err.clone())
        }
    }

    fn control_state(&self) -> NdnTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => NdnTaskControlState::Normal, 
            ControlStateImpl::Canceled => NdnTaskControlState::Canceled
        }
    }

    
    fn cancel(&self) -> BuckyResult<NdnTaskControlState> {
        let (tasks, waiters) = {
            let mut state = self.0.state.write().unwrap();
            let waiters = match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => {
                    let waiters = Some(waiters.transfer());
                    state.control_state = ControlStateImpl::Canceled;
                    waiters
                }, 
                _ => None
            };

            let tasks = match &mut state.task_state {
                TaskStateImpl::Uploading(uploading) => {
                    let tasks: Vec<Box<dyn UploadTask>> = uploading.running.iter().map(|t| t.clone_as_upload_task()).collect();
                    state.task_state = TaskStateImpl::Error(BuckyError::new(BuckyErrorCode::UserCanceled, "cancel invoked"));
                    tasks
                },
                _ => vec![]
            };

            (tasks, waiters)
        };

        if let Some(waiters) = waiters {
            waiters.wake();
        }

        for task in tasks {
            let _ = task.cancel();
        }
        
        Ok(NdnTaskControlState::Canceled)
    }

    fn close(&self) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Uploading(uploading) => {
                uploading.closed = true;
                if uploading.running.len() == 0 {
                    state.task_state = TaskStateImpl::Finished;
                }
            },
            _ => {}
        }
        Ok(())
    }

    fn cur_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Uploading(uploading) => uploading.history_speed.latest(),
            _ => 0
        }
    }


    fn history_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Uploading(uploading) => uploading.history_speed.average(),
            _ => 0
        }
    }
}

#[async_trait::async_trait]
impl UploadTask for UploadGroup {
    fn clone_as_upload_task(&self) -> Box<dyn UploadTask> {
        Box::new(self.clone())
    }

    fn add_task(&self, path: Option<String>, sub: Box<dyn UploadTask>) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Uploading(uploading) => {
                if !uploading.closed {
                    uploading.running.push(sub.clone_as_upload_task());
                    if let Some(path) = path {
                        if let Some(exists) = uploading.entries.insert(path, sub) {
                            let _ = exists.cancel();
                        }
                    }
                    Ok(())
                } else {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, ""))
                }
            },
            _ => Err(BuckyError::new(BuckyErrorCode::ErrorState, ""))
        }
    }

    fn sub_task(&self, path: &str) -> Option<Box<dyn UploadTask>> {
        if path.len() == 0 {
            Some(self.clone_as_upload_task())
        } else {
            let mut names = path.split("/");
            let name = names.next().unwrap();
    
            let state = self.0.state.read().unwrap(); 
            match &state.task_state {
                TaskStateImpl::Uploading(uploading) => {
                    let mut sub = uploading.entries.get(name).map(|t| t.clone_as_upload_task());
                    if sub.is_none() {
                        sub 
                    } else {
                        for name in names {
                            sub = sub.and_then(|t| t.sub_task(name));
                            if sub.is_none() {
                                break;
                            }
                        }
                        sub
                    }
                },
                _ => None
            }
        }
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.state.write().unwrap();
        let mut running = vec![];
        let mut cur_speed = 0;
        match &mut state.task_state {
            TaskStateImpl::Uploading(uploading) => {
                for sub in &uploading.running {
                    match sub.state() {
                        NdnTaskState::Finished | NdnTaskState::Error(_) => continue, 
                        _ => {
                            cur_speed += sub.calc_speed(when);
                            running.push(sub.clone_as_upload_task());
                        }
                    }  
                }
                uploading.history_speed.update(Some(cur_speed), when);
                if running.len() == 0 && uploading.closed {
                    state.task_state = TaskStateImpl::Finished;
                } else {
                    uploading.running = running;
                }
                cur_speed
            },
            _ => 0
        }
    }
}
