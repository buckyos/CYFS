use cyfs_base::*;
use cyfs_lib::*;
use cyfs_debug::Mutex;
use cyfs_perf_base::*;

use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::sync::Arc;
use chrono::{Datelike, Timelike, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfType {
    Requests,
    Accumulations,
    Actions,
    Records,

}

impl fmt::Display for PerfType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PerfType::Requests => write!(f, "Requests"),
            PerfType::Accumulations => write!(f, "Accumulations"),
            PerfType::Actions => write!(f, "Actions"),
            PerfType::Records => write!(f, "Records"),
        }
    }
}

#[derive(Clone)]
struct PerfIsolateInner {
    isolate_id: String,
    pending_reqs: HashMap<String, u64>,
    // 本地缓存对象
    request: HashMap<String, Vec<PerfRequestItem>>,
    accumulations: HashMap<String, Vec<PerfAccumulationItem>>,
    records: HashMap<String, Vec<PerfRecordItem>>,

    actions:  HashMap<String, Vec<PerfActionItem>>,
}

// 一个统计实体
pub struct PerfIsolateEntity {
    pub isolate_id: String,

    pub actions: HashMap<String, Vec<PerfActionItem>>,

    pub records: HashMap<String, Vec<PerfRecordItem>>,

    pub accumulations: HashMap<String, Vec<PerfAccumulationItem>>,

    pub request: HashMap<String, Vec<PerfRequestItem>>,
}

impl PerfIsolateInner {
    pub fn new(isolate_id: &str) -> PerfIsolateInner {
        Self {
            isolate_id: isolate_id.to_owned(),
            pending_reqs: HashMap::new(),
            request: HashMap::new(),
            accumulations: HashMap::new(),
            records: HashMap::new(),
            actions: HashMap::new(),
        }
    }

    pub fn begin_request(&mut self, id: &str, key: &str) {
        let full_id = format!("{}_{}", id, key);

        match self.pending_reqs.entry(full_id) {
            Entry::Vacant(v) => {
                let bucky_time = bucky_time_now();
                v.insert(bucky_time);
            }
            Entry::Occupied(_o) => {
                //unreachable!("perf request item already begin! id={}, key={}", id, key);
            }
        }
    }

    pub fn end_request(&mut self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u64>) {
        let now = bucky_time_now();
        let full_id = format!("{}_{}", id, key);
        match self.pending_reqs.remove(&full_id) {
            Some(tick) => {
                let during = if now > tick {
                    now - tick
                } else {
                    0
                };

                match self.request.entry(id.to_owned()) {
                    Entry::Vacant(v) => {
                        let mut req = PerfRequestItem { 
                            time: Utc::now().timestamp() as u64, 
                            spend_time: during,
                            err, 
                            stat: bytes,
                        };

                        v.insert(vec![req]);
                    }
                    Entry::Occupied(mut o) => {
                        let item = o.get_mut();
                        let mut req = PerfRequestItem { 
                            time: now, 
                            spend_time: during,
                            err, 
                            stat: bytes,
                        };
                        
                        item.push(req);

                    }
                }
            }
            None => {
                //unreachable!();
            }
        }

    }

    pub fn acc(&mut self, id: &str, err: BuckyErrorCode, size: u64) {
        match self.accumulations.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let acc = PerfAccumulationItem { 
                    time: Utc::now().timestamp() as u64, 
                    err, 
                    stat: size,
                };

                v.insert(vec![acc]);
            }
            Entry::Occupied(mut o) => {
                let item = o.get_mut();
                let mut acc = PerfAccumulationItem { 
                    time: Utc::now().timestamp() as u64, 
                    err, 
                    stat: size,
                };
                
                item.push(acc);

            }
        }

    }

    pub fn action(
        &mut self,
        id: &str,
        err: BuckyErrorCode,
        key: impl Into<String>,
        value: impl Into<String>,
    ) {
        match self.actions.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let action = PerfActionItem { 
                    time: Utc::now().timestamp() as u64, 
                    err, 
                    key: key.into(),
                    value: value.into(),
                };

                v.insert(vec![action]);
            }
            Entry::Occupied(mut o) => {
                let item = o.get_mut();
                let action = PerfActionItem { 
                    time: Utc::now().timestamp() as u64, 
                    err, 
                    key: key.into(),
                    value: value.into(),
                };
                
                item.push(action);

            }
        }
        
    }

    pub fn record(&mut self, id: &str, total: u64, total_size: Option<u64>) {
        match self.records.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let record = PerfRecordItem { 
                    time: Utc::now().timestamp() as u64, 
                    total,
                    total_size,
                };
                v.insert(vec![record]);
            }
            Entry::Occupied(mut o) => {
                let v = o.get_mut();
                let record = PerfRecordItem { 
                    time: Utc::now().timestamp() as u64, 
                    total,
                    total_size,
                };
                v.push(record);

            }
        }
    }

    // 取走所有已有的统计项
    pub fn take_data(&mut self) -> PerfIsolateEntity {
        let mut other = PerfIsolateEntity {
            isolate_id: self.isolate_id.to_owned(),
            request: HashMap::new(),
            accumulations: HashMap::new(),
            actions: HashMap::new(),
            records: HashMap::new(),
        };

        std::mem::swap(&mut self.actions, &mut other.actions);
        std::mem::swap(&mut self.records, &mut other.records);
        std::mem::swap(&mut self.accumulations, &mut other.accumulations);
        std::mem::swap(&mut self.request, &mut other.request);

        other
    }

}


#[derive(Clone)]
pub struct PerfIsolate(Arc<Mutex<PerfIsolateInner>>);

impl PerfIsolate {
    pub fn new(isolate_id: &str) -> Self {
        Self(Arc::new(Mutex::new(PerfIsolateInner::new(isolate_id))))
    }

    // 开启一个request
    pub fn begin_request(&self, id: &str, key: &str) {
        self.0.lock().unwrap().begin_request(id, key)
    }
    // 统计一个操作的耗时, 流量统计
    pub fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u64>) {
        self.0.lock().unwrap().end_request(id, key, err, bytes)
    }

    pub fn acc(&self, id: &str, err: BuckyErrorCode, size: u64) {
        self.0.lock().unwrap().acc(id, err, size)
    }

    pub fn action(
        &self,
        id: &str,
        err: BuckyErrorCode,
        name: impl Into<String>,
        value: impl Into<String>,
    ){
        self.0.lock().unwrap().action(id, err, name, value)
    }

    pub fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        self.0.lock().unwrap().record(id, total, total_size)
    }

    // 取走数据并置空
    pub(crate) fn take_data(&self) -> PerfIsolateEntity {
        self.0.lock().unwrap().take_data()
    }

    pub fn get_id(&self) -> String {
        self.0.lock().unwrap().isolate_id.clone()
    }
}