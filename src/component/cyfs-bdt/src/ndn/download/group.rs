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
    entries: HashMap<String, Box<dyn DownloadTask>>, 
    running: Vec<Box<dyn DownloadTask>>, 
    history_speed: HistorySpeed, 
    drain_score: i64, 
    control_state: DownloadTaskControlState
}

struct TaskImpl {
    priority: DownloadTaskPriority, 
    state: RwLock<StateImpl>
}

#[derive(Clone)]
pub struct DownloadGroup(Arc<TaskImpl>);

impl DownloadGroup {
    pub fn new(history_speed: HistorySpeedConfig, priority: Option<DownloadTaskPriority>) -> Self {
        Self(Arc::new(TaskImpl {
            priority: priority.unwrap_or_default(), 
            state: RwLock::new(StateImpl { 
                entries: Default::default(), 
                running: Default::default(), 
                history_speed: HistorySpeed::new(0, history_speed), 
                drain_score: 0, 
                control_state: DownloadTaskControlState::Normal
            })
        }))
    }

    pub fn add(&self, path: Option<String>, sub: Box<dyn DownloadTask>) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        state.running.push(sub.clone_as_task());
        if let Some(path) = path {
            state.entries.insert(path, sub);
        }
        Ok(())
    }
}

impl DownloadTask for DownloadGroup {
    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        DownloadTaskState::Downloading(0, 0.0)
    }

    fn control_state(&self) -> DownloadTaskControlState {
        self.0.state.read().unwrap().control_state.clone()
    }

    fn priority_score(&self) -> u8 {
        self.0.priority as u8
    }

    fn sub_task(&self, path: &str) -> Option<Box<dyn DownloadTask>> {
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
                DownloadTaskState::Finished | DownloadTaskState::Error(_) => continue, 
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

    fn drain_score(&self) -> i64 {
        self.0.state.read().unwrap().drain_score
    }

    fn on_drain(&self, expect_speed: u32) -> u32 {
        let running: Vec<Box<dyn DownloadTask>> = {
            self.0.state.read().unwrap().running.iter().map(|t| t.clone_as_task()).collect()
        };
        let mut new_expect = 0;
        let total: f64 = running.iter().map(|t| t.drain_score() as f64).sum();
        let score_cent = expect_speed as f64 / total;
        for task in running {
            new_expect += task.on_drain((task.priority_score() as f64 * score_cent) as u32);
        }

        {
            let mut state = self.0.state.write().unwrap();
            state.drain_score += new_expect as i64 - expect_speed as i64;
            state.running.sort_by(|l, r| r.drain_score().cmp(&l.drain_score()));
        }
        new_expect
    }
}
