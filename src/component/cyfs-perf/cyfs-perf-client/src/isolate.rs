use cyfs_base::*;
use cyfs_util::*;
use cyfs_debug::Mutex;
use cyfs_perf_base::*;

use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::sync::Arc;
use chrono::{Utc};

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

struct PerfIsolateInner {
    id: String,
    pending_reqs: HashMap<String, u64>,
    // 本地缓存对象
    requests: HashMap<String, Vec<PerfRequestItem>>,
    accumulations: HashMap<String, Vec<PerfAccumulationItem>>,
    records: HashMap<String, Vec<PerfRecordItem>>,

    actions:  HashMap<String, Vec<PerfActionItem>>,
}


pub struct PerfObject {
    id: String,
    // 本地缓存对象
    requests: HashMap<String, Vec<PerfRequestItem>>,
    accumulations: HashMap<String, Vec<PerfAccumulationItem>>,
    records: HashMap<String, Vec<PerfRecordItem>>,

    actions:  HashMap<String, Vec<PerfActionItem>>,
}

impl PerfObject {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            requests: HashMap::new(),
            accumulations: HashMap::new(),
            records: HashMap::new(),
            actions: HashMap::new(),
        }
    }

    pub fn requests(&self) -> &HashMap<String, Vec<PerfRequestItem>> {
        &self.requests
    }

    pub fn accumulations(&self) -> &HashMap<String, Vec<PerfAccumulationItem>> {
        &self.accumulations
    }

    pub fn actions(&self) -> &HashMap<String, Vec<PerfActionItem>> {
        &self.actions
    }

    pub fn records(&self) -> &HashMap<String, Vec<PerfRecordItem>> {
        &self.records
    }
}


impl PerfIsolateInner {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            pending_reqs: HashMap::new(),
            requests: HashMap::new(),
            accumulations: HashMap::new(),
            records: HashMap::new(),
            actions: HashMap::new(),
        }
    }

    // 开启一个request
    fn begin_request(&mut self, id: &str, key: &str) {
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
    // 统计一个操作的耗时, 流量统计
    fn end_request(&mut self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        let now = bucky_time_now();
        let full_id = format!("{}_{}", id, key);
        match self.pending_reqs.remove(&full_id) {
            Some(tick) => {
                let during = if now > tick {
                    now - tick
                } else {
                    0
                };

                match self.requests.entry(id.to_owned()) {
                    Entry::Vacant(v) => {
                        let req = PerfRequestItem { 
                            time: Utc::now().timestamp() as u64, 
                            spend_time: during,
                            err, 
                            stat: bytes,
                        };

                        v.insert(vec![req]);
                    }
                    Entry::Occupied(mut o) => {
                        let item = o.get_mut();
                        let req = PerfRequestItem { 
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

    fn acc(&mut self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
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
                let acc = PerfAccumulationItem { 
                    time: Utc::now().timestamp() as u64, 
                    err, 
                    stat: size,
                };
                
                item.push(acc);

            }
        }
    }

    fn action(
        &mut self,
        id: &str,
        err: BuckyErrorCode,
        name: String,
        value: String,
    ){
        match self.actions.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let action = PerfActionItem { 
                    time: Utc::now().timestamp() as u64, 
                    err, 
                    key: name.into(),
                    value: value.into(),
                };

                v.insert(vec![action]);
            }
            Entry::Occupied(mut o) => {
                let item = o.get_mut();
                let action = PerfActionItem { 
                    time: Utc::now().timestamp() as u64, 
                    err, 
                    key: name.into(),
                    value: value.into(),
                };
                
                item.push(action);

            }
        }
    }

    fn record(&mut self, id: &str, total: u64, total_size: Option<u64>) {
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

    // 取走数据并置空
    pub fn take_data(&mut self) -> PerfObject {
        let mut other = PerfObject::new(self.id.to_owned());

        std::mem::swap(&mut self.actions, &mut other.actions);
        std::mem::swap(&mut self.records, &mut other.records);
        std::mem::swap(&mut self.accumulations, &mut other.accumulations);
        std::mem::swap(&mut self.requests, &mut other.requests);

        other
    }


}


#[derive(Clone)]
pub struct PerfIsolate(Arc<Mutex<PerfIsolateInner>>);

impl PerfIsolate {
    pub fn new(id: &str) -> Self {
        Self(Arc::new(Mutex::new(PerfIsolateInner::new(id))))
    }

    // 取走数据并置空
    pub(crate) fn take_data(&self) -> PerfObject {
        self.0.lock().unwrap().take_data()
    }

}

impl Perf for PerfIsolate {

    fn get_id(&self) -> String {
        self.0.lock().unwrap().id.clone()
    }
    // create a new perf module
    fn fork(&self, id: &str) -> BuckyResult<Box<dyn Perf>> {
        Ok(Box::new(Self(Arc::new(Mutex::new(PerfIsolateInner::new(id))))))
    }

    fn begin_request(&self, id: &str, key: &str) {
        self.0.lock().unwrap().begin_request(id, key)
    }

    fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        self.0.lock().unwrap().end_request(id, key, err, bytes)
    }

    fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        self.0.lock().unwrap().acc(id, err, size)
    }

    fn action(
        &self,
        id: &str,
        err: BuckyErrorCode,
        name: String,
        value: String,
    ) {
        self.0.lock().unwrap().action(id, err, name, value)
    }

    fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        self.0.lock().unwrap().record(id, total, total_size)
    }

}