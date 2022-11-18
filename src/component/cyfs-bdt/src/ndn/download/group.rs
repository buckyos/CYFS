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
    drain_score: i64, 
}

enum TaskStateImpl {
    Downloading(DownloadingState), 
    Finished, 
    Error(BuckyErrorCode), 
}

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

struct StateImpl {
    task_state: TaskStateImpl, 
    control_state: ControlStateImpl
}

struct TaskImpl {
    priority: DownloadTaskPriority, 
    history_speed: HistorySpeedConfig, 
    state: RwLock<StateImpl>
}

#[derive(Clone)]
pub struct DownloadGroup(Arc<TaskImpl>);

impl DownloadGroup {
    pub fn new(
        history_speed: HistorySpeedConfig, 
        priority: Option<DownloadTaskPriority>, 
    ) -> Self {
        Self(Arc::new(TaskImpl {
            priority: priority.unwrap_or_default(), 
            history_speed: history_speed.clone(), 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Downloading(DownloadingState {
                    entries: Default::default(), 
                    running: Default::default(), 
                    history_speed: HistorySpeed::new(0, history_speed), 
                    drain_score: 0, 
                    closed: false, 
                }),
                control_state: ControlStateImpl::Normal(StateWaiter::new())
            })
        }))
    }

    pub fn create_sub_group(
        &self, 
        path: String, 
    ) -> BuckyResult<Box<dyn DownloadTask>> {
        if let Some(group) = self.sub_task(path.as_str()) {
            Ok(group)
        } else {
            let parts = path.split("/");
            let mut parent = Self.clone_as_task();
            
            for part in parts {
                if let Some(sub) = parent.sub_task(part) {
                    parent = sub;
                } else {
                    let sub = DownloadGroup::new(self.0.history_speed.clone(), None);
                    parent.add_task(Some(part.to_owned()), sub.clone_as_task())?;
                    parent = sub.clone_as_task();
                }
            }

            Ok(parent)
        }
    }

    pub fn makesure_path(
        &self, 
        path: Option<String>
    ) -> BuckyResult<(Box<dyn DownloadTask>, Option<String>)> {
        if let Some(group) = path {
            if group.len() == 0 {
                return Ok((self.clone_as_task(), None));
            } 

            let mut parts: Vec<&str> = group.split("/").collect();
            if parts.len() == 0 {
                return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "invalid group path"))
            } 
            
            let last_part = if parts[parts.len() - 1].len() == 0 {
                None 
            } else {
                Some(parts[parts.len() - 1].to_owned())
            };

            parts.remove(parts.len() - 1);

            let group_path = parts.join("/"); 
            Ok((self.create_sub_group(group_path)?, last_part))
        } else {
            Ok((self.clone_as_task(), None))
        }
    }


    pub fn create_sub_task(&self, path: Option<String>, task: &dyn DownloadTask) -> BuckyResult<()> {
        let (owner, path) = self.makesure_path(path)?;
        let _ = owner.add_task(path, task.clone_as_task())?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl DownloadTask for DownloadGroup {
    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        DownloadTaskState::Downloading(0, 0.0)
    }

    fn control_state(&self) -> DownloadTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => DownloadTaskControlState::Normal, 
            ControlStateImpl::Canceled => DownloadTaskControlState::Canceled
        }
    }

    fn priority_score(&self) -> u8 {
        self.0.priority as u8
    }

    fn add_task(&self, path: Option<String>, sub: Box<dyn DownloadTask>) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                if !downloading.closed {
                    downloading.running.push(sub.clone_as_task());
                    if let Some(path) = path {
                        downloading.entries.insert(path, sub);
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
            let mut names = path.split("::");
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

    fn drain_score(&self) -> i64 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.drain_score,
            _ => 0
        }
    }

    fn on_drain(&self, expect_speed: u32) -> u32 {
        let running: Vec<Box<dyn DownloadTask>> = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => downloading.running.iter().map(|t| t.clone_as_task()).collect(),
                _ => vec![]
            }
        };


        let mut new_expect = 0;
        let total: f64 = running.iter().map(|t| t.drain_score() as f64).sum();
        let score_cent = expect_speed as f64 / total;
        for task in running {
            new_expect += task.on_drain((task.priority_score() as f64 * score_cent) as u32);
        }

        {
            let mut state = self.0.state.write().unwrap();
            match &mut state.task_state {
                TaskStateImpl::Downloading(downloading) => {
                    downloading.drain_score += new_expect as i64 - expect_speed as i64;
                    downloading.running.sort_by(|l, r| r.drain_score().cmp(&l.drain_score()));
                    new_expect
                },
                _ => 0
            }
        }
    }


    fn cancel(&self) -> BuckyResult<DownloadTaskControlState> {
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
                    state.task_state = TaskStateImpl::Error(BuckyErrorCode::UserCanceled);
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
