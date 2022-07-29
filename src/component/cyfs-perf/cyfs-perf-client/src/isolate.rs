use cyfs_base::*;
use cyfs_lib::SharedCyfsStack;
use cyfs_util::*;
use cyfs_debug::Mutex;
use cyfs_perf_base::*;

use std::collections::HashMap;
use std::fmt;
use std::ops::DerefMut;
use std::sync::Arc;

use crate::{PerfServerConfig};
use crate::manager::{IsolateManager, IsolateManagerRef};

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

// 临时统计数据
pub struct PerfIsolateEntity {
    pub isolate_id: String,
    pub requests: HashMap<String, Vec<PerfRequestItem>>,
    pub accumulations: HashMap<String, Vec<PerfAccumulationItem>>,
    pub records: HashMap<String, Vec<PerfRecordItem>>,
    pub actions:  HashMap<String, Vec<PerfActionItem>>,
}

pub struct PerfIsolateInner {
    isolate_id: String,
    perf_server_config: PerfServerConfig,
    span_time: u32,
    dec_id: ObjectId,

    stack: SharedCyfsStack,

    pending_reqs: Mutex<HashMap<String, u64>>,
    // 本地缓存对象
    requests: Mutex<HashMap<String, Vec<PerfRequestItem>>>,
    accumulations: Mutex<HashMap<String, Vec<PerfAccumulationItem>>>,
    records: Mutex<HashMap<String, Vec<PerfRecordItem>>>,
    actions:  Mutex<HashMap<String, Vec<PerfActionItem>>>,

    // IsolateManager
    manager: IsolateManagerRef
}

impl PerfIsolateInner {
    pub fn new(
        isolate_id: impl Into<String>,
        span_time: u32,
        dec_id: ObjectId,
        perf_server_config: PerfServerConfig,
        manager: Arc<IsolateManager>,
        stack: SharedCyfsStack) -> Self {
        Self {
            isolate_id: isolate_id.into(),
            span_time,
            dec_id,
            perf_server_config,
            stack,
            pending_reqs: Mutex::new(HashMap::new()),
            requests: Mutex::new(HashMap::new()),
            accumulations: Mutex::new(HashMap::new()),
            records: Mutex::new(HashMap::new()),
            actions: Mutex::new(HashMap::new()),
            manager
        }
    }

    // 取走所有已有的统计项
    pub fn take_data(&self) -> PerfIsolateEntity {
        let mut other = PerfIsolateEntity {
            isolate_id: self.isolate_id.to_owned(),
            requests: HashMap::new(),
            accumulations: HashMap::new(),
            records: HashMap::new(),
            actions: HashMap::new(),
        };

        let reqs = std::mem::replace(self.requests.lock().unwrap().deref_mut(), HashMap::new());

        let acc = std::mem::replace(self.accumulations.lock().unwrap().deref_mut(), HashMap::new());

        let act = std::mem::replace(self.actions.lock().unwrap().deref_mut(), HashMap::new());

        let rec = std::mem::replace(self.records.lock().unwrap().deref_mut(), HashMap::new());

        other.requests = reqs;
        other.accumulations = acc;
        other.actions = act;
        other.records = rec;

        other
    }

    fn get_id(&self) -> String {
        self.isolate_id.clone()
    }

    // 开启一个request
    fn begin_request(&self, id: &str, key: &str) {
        let full_id = format!("{}_{}", id, key);
        let mut pending_reqs = self.pending_reqs.lock().unwrap();
        if !pending_reqs.contains_key(&full_id) {
            pending_reqs.insert(full_id, bucky_time_now());
        }
    }
    // 统计一个操作的耗时, 流量统计
    fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        let mut pending_reqs = self.pending_reqs.lock().unwrap();
        let full_id = format!("{}_{}", id, key);
        if let Some(tick) = pending_reqs.remove(&full_id) {
            let now = bucky_time_now();
            let during = if now > tick {
                now - tick
            } else {
                0
            };

            let mut requests = self.requests.lock().unwrap();
            let items = requests.entry(id.to_owned()).or_insert(vec![]);
            items.push(PerfRequestItem {
                time: now,
                spend_time: during,
                err,
                stat: bytes,
            });
        }
    }

    fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        let mut accumulations = self.accumulations.lock().unwrap();
        let accs = accumulations.entry(id.to_owned()).or_insert(vec![]);
        accs.push(PerfAccumulationItem {
            time: bucky_time_now(),
            err,
            stat: size,
        });
    }

    fn action(
        &self,
        id: &str,
        err: BuckyErrorCode,
        name: String,
        value: String,
    ){
        let mut actions = self.actions.lock().unwrap();
        let items = actions.entry(id.to_owned()).or_insert(vec![]);
        items.push(PerfActionItem {
            time: bucky_time_now(),
            err,
            key: name.into(),
            value: value.into(),
        })
    }

    fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        let mut records = self.records.lock().unwrap();

        let items = records.entry(id.to_owned()).or_insert(vec![]);
        items.push(PerfRecordItem {
            time: bucky_time_now(),
            total,
            total_size,
        });
    }
}

#[derive(Clone)]
pub struct PerfIsolate(Arc<PerfIsolateInner>);

impl PerfIsolate {
    pub fn new(
        id: &str,
        span_time: u32,
        dec_id: ObjectId,
        perf_server_config: PerfServerConfig,
        stack: SharedCyfsStack,) -> Self {            
        let ret = PerfIsolateInner::new(
            id, span_time, dec_id,
            perf_server_config, IsolateManager::new(stack.clone(), dec_id, span_time),stack);
        Self(Arc::new(ret))
    }

    pub async fn start(&self) {
        let manager = self.0.manager.clone();
       async_std::task::spawn(async move {
            manager.start().await;
        });
    }

    // 取走数据并置空
    pub fn take_data(&self) -> PerfIsolateEntity {
        self.0.take_data()
    }

    pub fn fork_self(&self, id: &str) -> Self {
        Self(Arc::new(PerfIsolateInner::new(
            id,
            self.0.span_time,
            self.0.dec_id.clone(),
            self.0.perf_server_config.clone(),
            self.0.manager.clone(),
            self.0.stack.clone()
        )))
    }
}

impl Perf for PerfIsolate {

    fn get_id(&self) -> String {
        self.0.get_id()
    }
    // create a new perf module
    fn fork(&self, id: &str) -> BuckyResult<Box<dyn Perf>> {
        let ret = self.0.manager.fork(id, self);
        if ret.is_none() {
            return Err(BuckyError::from("fork get rwlock failed"));
        }

        Ok(Box::new(ret.unwrap()))
    }

    fn begin_request(&self, id: &str, key: &str) {
        self.0.begin_request(id, key)
    }

    fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        self.0.end_request(id, key, err, bytes)
    }

    fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        self.0.acc(id, err, size)
    }

    fn action(
        &self,
        id: &str,
        err: BuckyErrorCode,
        name: String,
        value: String,
    ) {
        self.0.action(id, err, name, value)
    }

    fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        self.0.record(id, total, total_size)
    }

}
