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
    history_downloaded: u64, 
    downloaded: u64, 
    history_speed: HistorySpeed, 
}

enum TaskStateImpl {
    Downloading(DownloadingState), 
    Finished(u64), 
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
                    history_downloaded: 0, 
                    downloaded: 0, 
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


impl NdnTask for DownloadGroup {
    fn clone_as_task(&self) -> Box<dyn NdnTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> NdnTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Downloading(_) => NdnTaskState::Running,
            TaskStateImpl::Finished(_) => NdnTaskState::Finished, 
            TaskStateImpl::Error(err) => NdnTaskState::Error(err.clone())
        }
        
    }

    fn control_state(&self) -> NdnTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => NdnTaskControlState::Normal, 
            ControlStateImpl::Canceled => NdnTaskControlState::Canceled
        }
    }

    fn close(&self, recursion: bool) -> BuckyResult<()> {
        let children: Option<Vec<_>> = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.task_state {
                TaskStateImpl::Downloading(downloading) => {
                    let running = if recursion {
                        Some(downloading.running.iter().map(|t| t.clone_as_download_task()).collect())
                    } else {
                        None
                    };
                    downloading.closed = true;
                    if downloading.running.len() == 0 {
                        state.task_state = TaskStateImpl::Finished(downloading.downloaded);
                    }
                    running
                },
                _ => None
            }
        };

        if recursion {
            for task in children.unwrap() {
                let _ = task.close(recursion);
            }
        }
        
        Ok(())
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

    fn transfered(&self) -> u64 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.downloaded,
            TaskStateImpl::Finished(downloaded) => *downloaded, 
            _ => 0
        }
    }


    fn cancel_by_error(&self, err: BuckyError) -> BuckyResult<NdnTaskControlState> {
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
                    let tasks: Vec<Box<dyn DownloadTask>> = downloading.running.iter().map(|t| t.clone_as_download_task()).collect();
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
        
        Ok(NdnTaskControlState::Canceled)
    }
}


#[async_trait::async_trait]
impl DownloadTask for DownloadGroup {
    fn clone_as_download_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn add_task(&self, path: Option<String>, sub: Box<dyn DownloadTask>) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                if !downloading.closed {
                    downloading.running.push(sub.clone_as_download_task());
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
            Some(self.clone_as_download_task())
        } else {
            let mut names = path.split("/");
            let name = names.next().unwrap();
    
            let state = self.0.state.read().unwrap(); 
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => {
                    let mut sub = downloading.entries.get(name).map(|t| t.clone_as_download_task());
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
        let mut running_downloaded = 0;
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                for sub in &downloading.running {
                    cur_speed += sub.calc_speed(when);
                    match sub.state() {
                        NdnTaskState::Finished | NdnTaskState::Error(_) => {
                            downloading.history_downloaded += sub.transfered();
                        }, 
                        _ => {
                            running_downloaded += sub.transfered();
                            running.push(sub.clone_as_download_task());
                        }
                    }  
                }
                downloading.downloaded = downloading.history_downloaded + running_downloaded;
                downloading.history_speed.update(Some(cur_speed), when);
                if running.len() == 0 && downloading.closed {
                    state.task_state = TaskStateImpl::Finished(downloading.downloaded);
                } else {
                    downloading.running = running;
                }
                cur_speed
            },
            _ => 0
        }
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
