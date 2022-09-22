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

struct StateImpl {
    entries: HashMap<String, Box<dyn UploadTask>>, 
    running: Vec<Box<dyn UploadTask>>, 
    history_speed: HistorySpeed, 
    control_state: UploadTaskControlState
}

struct TaskImpl {
    priority: UploadTaskPriority, 
    state: RwLock<StateImpl>
}

#[derive(Clone)]
pub struct UploadGroup(Arc<TaskImpl>);

impl UploadGroup {
    pub fn new(history_speed: HistorySpeedConfig, priority: Option<UploadTaskPriority>) -> Self {
        Self(Arc::new(TaskImpl {
            priority: priority.unwrap_or_default(), 
            state: RwLock::new(StateImpl { 
                entries: Default::default(), 
                running: Default::default(), 
                history_speed: HistorySpeed::new(0, history_speed), 
                control_state: UploadTaskControlState::Normal
            })
        }))
    }
}

impl UploadTask for UploadGroup {
    fn clone_as_task(&self) -> Box<dyn UploadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> UploadTaskState {
        UploadTaskState::Uploading(0)
    }

    fn control_state(&self) -> UploadTaskControlState {
        self.0.state.read().unwrap().control_state.clone()
    }

    fn priority_score(&self) -> u8 {
        self.0.priority as u8
    }

    fn add_task(&self, path: Option<String>, sub: Box<dyn UploadTask>) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        state.running.push(sub.clone_as_task());
        if let Some(path) = path {
            state.entries.insert(path, sub);
        }
        Ok(())
    }

    fn sub_task(&self, path: &str) -> Option<Box<dyn UploadTask>> {
        let mut names = path.split("::");
        let name = names.next().unwrap();

        let mut sub = self.0.state.read().unwrap().entries.get(name).map(|t| t.clone_as_task());
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
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.state.write().unwrap();
        let mut running = vec![];
        let mut cur_speed = 0;
        for sub in &state.running {
            match sub.state() {
                UploadTaskState::Finished | UploadTaskState::Error(_) => continue, 
                _ => {
                    cur_speed += sub.calc_speed(when);
                    running.push(sub.clone_as_task());
                }
            }  
        }
        state.history_speed.update(Some(cur_speed), when);
        state.running = running;
        cur_speed
    }

    fn cur_speed(&self) -> u32 {
        self.0.state.read().unwrap().history_speed.latest()
    }

    fn history_speed(&self) -> u32 {
        self.0.state.read().unwrap().history_speed.average()
    }
}
