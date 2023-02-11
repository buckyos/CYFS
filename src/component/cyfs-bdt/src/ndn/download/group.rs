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

struct DownloadingState {
    entries: HashMap<String, Box<dyn DownloadTask>>, 
    running: Vec<Box<dyn DownloadTask>>, 
    closed: bool, 
    history_speed: HistorySpeed, 
}

enum TaskStateImpl {
    Downloading(DownloadingState), 
    Finished, 
    Error(BuckyError), 
}

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

struct StateImpl {
    task_state: TaskStateImpl, 
    control_state: ControlStateImpl, 
}

struct TaskImpl {
    history_speed: HistorySpeedConfig, 
    state: RwLock<StateImpl>
}

#[derive(Clone)]
pub struct DownloadGroup(Arc<TaskImpl>);

impl DownloadGroup {
    pub fn new(history_speed: HistorySpeedConfig) -> Self {
        Self(Arc::new(TaskImpl {
            history_speed: history_speed.clone(), 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Downloading(DownloadingState {
                    entries: Default::default(), 
                    running: Default::default(), 
                    history_speed: HistorySpeed::new(0, history_speed), 
                    closed: false, 
                }),
                control_state: ControlStateImpl::Normal(StateWaiter::new()), 
            })
        }))
    }

    pub fn history_config(&self) -> &HistorySpeedConfig {
        &self.0.history_speed
    }
}

#[async_trait::async_trait]
impl DownloadTask for DownloadGroup {
    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Downloading(_) => DownloadTaskState::Downloading, 
            TaskStateImpl::Finished => DownloadTaskState::Finished, 
            TaskStateImpl::Error(err) => DownloadTaskState::Error(err.clone())
        }
        
    }

    fn control_state(&self) -> DownloadTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => DownloadTaskControlState::Normal, 
            ControlStateImpl::Canceled => DownloadTaskControlState::Canceled
        }
    }

    fn add_task(&self, path: Option<String>, sub: Box<dyn DownloadTask>) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                if !downloading.closed {
                    downloading.running.push(sub.clone_as_task());
                    if let Some(path) = path {
                        if let Some(exists) = downloading.entries.insert(path, sub) {
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

    fn sub_task(&self, path: &str) -> Option<Box<dyn DownloadTask>> {
        if path.len() == 0 {
            Some(self.clone_as_task())
        } else {
            let mut names = path.split("/");
            let name = names.next().unwrap();
    
            let state = self.0.state.read().unwrap(); 
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => {
                    let mut sub = downloading.entries.get(name).map(|t| t.clone_as_task());
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

    fn close(&self) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                downloading.closed = true;
                if downloading.running.len() == 0 {
                    state.task_state = TaskStateImpl::Finished;
                }
            },
            _ => {}
        }
        Ok(())
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.state.write().unwrap();
        let mut running = vec![];
        let mut cur_speed = 0;
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                for sub in &downloading.running {
                    match sub.state() {
                        DownloadTaskState::Finished | DownloadTaskState::Error(_) => continue, 
                        _ => {
                            cur_speed += sub.calc_speed(when);
                            running.push(sub.clone_as_task());
                        }
                    }  
                }
                downloading.history_speed.update(Some(cur_speed), when);
                if running.len() == 0 && downloading.closed {
                    state.task_state = TaskStateImpl::Finished;
                } else {
                    downloading.running = running;
                }
                cur_speed
            },
            _ => 0
        }
    }

    fn cur_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.history_speed.latest(),
            _ => 0
        }
    }

    fn history_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.history_speed.average(),
            _ => 0
        }
    }

    fn cancel_by_error(&self, err: BuckyError) -> BuckyResult<DownloadTaskControlState> {
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
                TaskStateImpl::Downloading(downloading) => {
                    let tasks: Vec<Box<dyn DownloadTask>> = downloading.running.iter().map(|t| t.clone_as_task()).collect();
                    state.task_state = TaskStateImpl::Error(err.clone());
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
            let _ = task.cancel_by_error(err.clone());
        }
        
        Ok(DownloadTaskControlState::Canceled)
    }

    async fn wait_user_canceled(&self) -> BuckyError {
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => Some(waiters.new_waiter()), 
                _ => None
            }
        };
        
        
        if let Some(waiter) = waiter {
            let _ = StateWaiter::wait(waiter, || self.control_state()).await;
        } 

        BuckyError::new(BuckyErrorCode::UserCanceled, "")
    }
}
