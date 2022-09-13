use std::{
    collections::{LinkedList}, 
    sync::{Arc, RwLock}, 
    time::Duration
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::{
    channel::*
};

#[derive(Clone)]
pub struct DownloadSource {
    pub target: DeviceId, 
    pub object_id: Option<ObjectId>, 
    pub encode_desc: PieceSessionType, 
    pub referer: Option<String>
}



struct SingleContextImpl {
    referer: Option<String>, 
    sources: RwLock<LinkedList<DownloadSource>>, 
}

#[derive(Clone)]
pub struct SingleDownloadContext(Arc<SingleContextImpl>);

impl SingleDownloadContext {
    pub fn streams(referer: Option<String>, remotes: Vec<DeviceId>) -> Self {
        let mut sources = LinkedList::new();
        for remote in remotes {
            sources.push_back(DownloadSource {
                target: remote, 
                object_id: None, 
                encode_desc: PieceSessionType::Stream(0), 
                referer: None
            });
        } 
        Self(Arc::new(SingleContextImpl {
            referer, 
            sources: RwLock::new(sources)
        }))
    }

    pub fn referer(&self) -> Option<&str> {
        self.0.referer.as_ref().map(|s| s.as_str())
    }

    pub fn add_source(&self, source: DownloadSource) {
        self.0.sources.write().unwrap().push_back(source);
    }

    pub fn sources_of(&self, filter: impl Fn(&DownloadSource) -> bool, limit: usize) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        let mut count = 0;
        let sources = self.0.sources.read().unwrap();
        for source in sources.iter() {
            if filter(source) {
                result.push_back(DownloadSource {
                    target: source.target.clone(), 
                    object_id: source.object_id.clone(), 
                    encode_desc: source.encode_desc.clone(), 
                    referer: source.referer.clone().or(self.referer().map(|s| s.to_owned()))
                });
                count += 1;
                if count >= limit {
                    return result;
                }
            }
        }
        return result;
    }
}


struct MultiContextImpl {
    contexts: RwLock<LinkedList<SingleDownloadContext>>
}

#[derive(Clone)]
pub struct MultiDownloadContext(Arc<MultiContextImpl>);

impl MultiDownloadContext {
    pub fn new() -> Self {
        Self(Arc::new(MultiContextImpl {
            contexts: RwLock::new(LinkedList::new())
        }))
    }

    pub fn add_context(&self, context: SingleDownloadContext) {
        self.0.contexts.write().unwrap().push_back(context);
    }

    pub fn sources_of(&self, filter: impl Fn(&DownloadSource) -> bool + Copy, limit: usize) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        let mut limit = limit;
        let contexts = self.0.contexts.read().unwrap();
        for context in contexts.iter() {
            let mut part = context.sources_of(filter, limit);
            limit -= part.len();
            result.append(&mut part);
            if limit == 0 {
                break;
            }
        }   
        result
    }
}



#[derive(Clone)]
pub struct HistorySpeedConfig {
    pub attenuation: f64, 
    pub atomic: Duration, 
    pub expire: Duration
}

#[derive(Clone)]
// 计算历史速度的方法， 在过去的一段时间内，  Sum(speed(t)*(衰减^t))/样本数
pub struct HistorySpeed {
    expire_count: usize, 
    config: HistorySpeedConfig, 
    intermediate: LinkedList<f64>, 
    last_update: Timestamp
}

impl HistorySpeed {
    pub fn new(initial: u32, config: HistorySpeedConfig) -> Self {
        let mut intermediate = LinkedList::new();
        intermediate.push_back(initial as f64);

        Self {
            expire_count: (config.expire.as_micros() / config.atomic.as_micros()) as usize, 
            config, 
            intermediate, 
            last_update: bucky_time_now() 
        }   
    }

    pub fn update(&mut self, cur_speed: Option<u32>, when: Timestamp) {
        let cur_speed = cur_speed.unwrap_or(self.latest());

        if when > self.last_update {
            let count = ((when - self.last_update) / self.config.atomic.as_micros() as u64) as usize;

            for _ in 0..count {
                self.intermediate.iter_mut().for_each(|v| *v = (*v) * self.config.attenuation);
                self.intermediate.push_back(cur_speed as f64);
                if self.intermediate.len() > self.expire_count {
                    self.intermediate.pop_front();
                }
            }
        };
    }

    pub fn average(&self) -> u32 {
        let total: f64 = self.intermediate.iter().sum();
        (total / self.intermediate.len() as f64) as u32
    }

    pub fn latest(&self) -> u32 {
        self.intermediate.back().cloned().unwrap() as u32
    }

    pub fn config(&self) -> &HistorySpeedConfig {
        &self.config
    }
}


pub struct SpeedCounter {
    last_recv: u64, 
    last_update: Timestamp, 
    cur_speed: u32
}


impl SpeedCounter {
    pub fn new(init_recv: usize) -> Self {
        Self {
            last_recv: init_recv as u64, 
            last_update: bucky_time_now(), 
            cur_speed: 0
        }
    }

    pub fn on_recv(&mut self, recv: usize) {
        self.last_recv += recv as u64;
    }

    pub fn update(&mut self, when: Timestamp) -> u32 {
        if when > self.last_update {
            let last_recv = self.last_recv;
            self.last_recv = 0;
            self.cur_speed = ((last_recv * 1000 * 1000) as f64 / (when - self.last_update) as f64) as u32;
            self.cur_speed
        } else {
            self.cur_speed
        }
    }

    pub fn cur(&self) -> u32 {
        self.cur_speed
    }
}



#[derive(Clone, Copy)]
pub enum DownloadTaskPriority {
    Backgroud = 1, 
    Normal = 2, 
    Realtime = 4,
}

impl Default for DownloadTaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}


// 对scheduler的接口
#[derive(Debug)]
pub enum DownloadTaskState {
    Pending, 
    Downloading(u32/*速度*/, f32/*进度*/),
    Paused,
    Error(BuckyErrorCode/*被cancel的原因*/), 
    Finished
}

#[derive(Clone, Debug)]
pub enum DownloadTaskControlState {
    Normal, 
    Paused, 
    Canceled, 
}



pub trait DownloadTask2: Send + Sync {
    fn clone_as_task(&self) -> Box<dyn DownloadTask2>;
    fn state(&self) -> DownloadTaskState;
    fn control_state(&self) -> DownloadTaskControlState;

    fn priority_score(&self) -> u8 {
        DownloadTaskPriority::Normal as u8
    }
    fn sub_task(&self, _path: &str) -> Option<Box<dyn DownloadTask2>> {
        None
    }

    fn calc_speed(&self, when: Timestamp) -> u32;
    fn cur_speed(&self) -> u32;
    fn history_speed(&self) -> u32;

    fn drain_score(&self) -> i64 {
        0
    }
    fn on_drain(&self, expect_speed: u32) -> u32;
}