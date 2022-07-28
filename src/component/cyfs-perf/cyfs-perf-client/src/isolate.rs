use cyfs_base::*;
use cyfs_lib::{UtilGetZoneOutputRequest, SharedCyfsStack};
use cyfs_core::*;
use cyfs_util::*;
use cyfs_debug::Mutex;
use cyfs_perf_base::*;

use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::sync::Arc;

use async_std::prelude::*;
use std::time::Duration;

use crate::{PerfServerConfig};
use crate::store::PerfStore;

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

pub struct PerfIsolateInner {
    isolate_id: String,
    version: String,
    perf_server_config: PerfServerConfig,
    span_time: u32,
    dec_id: Option<ObjectId>,

    stack: SharedCyfsStack,

    pending_reqs: Mutex<HashMap<String, u64>>,
    // 本地缓存对象
    requests: Mutex<HashMap<String, Vec<PerfRequestItem>>>,
    accumulations: Mutex<HashMap<String, Vec<PerfAccumulationItem>>>,
    records: Mutex<HashMap<String, Vec<PerfRecordItem>>>,

    actions:  Mutex<HashMap<String, Vec<PerfActionItem>>>,
}

impl PerfIsolateInner {
    pub fn new(
        isolate_id: impl Into<String>,
        version: String,
        span_time: u32,
        dec_id: Option<ObjectId>,
        perf_server_config: PerfServerConfig,
        stack: SharedCyfsStack) -> Self {
        Self {
            isolate_id: isolate_id.into(),
            version,
            span_time,
            dec_id,
            perf_server_config,
            stack,
            pending_reqs: Mutex::new(HashMap::new()),
            requests: Mutex::new(HashMap::new()),
            accumulations: Mutex::new(HashMap::new()),
            records: Mutex::new(HashMap::new()),
            actions: Mutex::new(HashMap::new()),
        }
    }
    // 开启定期保存的任务
    async fn run_save(&self) -> BuckyResult<()>{
        // let device_id = stack.local_device_id();
        let device_id = self.stack.local_device().desc().calculate_id();
        let req = UtilGetZoneOutputRequest::new(None, None);
        let resp = self.stack.util().get_zone(req).await?;
        let people_id = resp.zone.owner().to_owned();
        let store = PerfStore::new(self.span_time, people_id, device_id, self.dec_id.clone(), self.stack.clone());
        
        let mut interval = async_std::stream::interval(Duration::from_secs(60));
        while let Some(_) = interval.next().await {

            let _ = self.inner_save(&store).await?;
        }

        Ok(())

    }
    
    async fn inner_save(&self, store: &PerfStore) -> BuckyResult<()> {
        let mut reqs = HashMap::new();
        {
            let mut requests = self.requests.lock().unwrap();
            for ( id, items) in requests.iter_mut() {
                let mut v1 = Vec::new();
                std::mem::swap(items, &mut v1);
                reqs.insert(id.to_owned(), v1);
            }
        }

        let mut acc = HashMap::new();
        {
            let mut accumulations = self.accumulations.lock().unwrap();
            for ( id, items) in accumulations.iter_mut() {
                let mut v1 = Vec::new();
                std::mem::swap(items, &mut v1);
                acc.insert(id.to_owned(), v1);
            }
        }

        let mut act = HashMap::new();
        {
            let mut actions = self.actions.lock().unwrap();
            for ( id, items) in actions.iter_mut() {
                let mut v1 = Vec::new();
                std::mem::swap(items, &mut v1);
                act.insert(id.to_owned(), v1);
            }
        }

        let mut rec = HashMap::new();
        {
            let mut records = self.records.lock().unwrap();
            for ( id, items) in records.iter_mut() {
                let mut v1 = Vec::new();
                std::mem::swap(items, &mut v1);
                rec.insert(id.to_owned(), v1);
            }
        }

        // FIXME:  futures::future::join_all parallel 
        store.request(&self.isolate_id, reqs).await?;
        store.acc(&self.isolate_id, acc).await?;
        store.action(&self.isolate_id, act).await?;
        store.record(&&self.isolate_id, rec).await?;

        Ok(())
    }



}

impl Perf for PerfIsolateInner {
    fn get_id(&self) -> String {
        self.isolate_id.clone()
    }
    // create a new perf module
    fn fork(&self, id: &str) -> BuckyResult<Box<dyn Perf>> {
        Ok(Box::new(Self {
            isolate_id: id.to_owned(),
            version: self.version.to_owned(),
            span_time: self.span_time,
            dec_id: self.dec_id.clone(),
            perf_server_config: self.perf_server_config.clone(),
            stack: self.stack.clone(),
            
            pending_reqs: Mutex::new(HashMap::new()),
            requests: Mutex::new(HashMap::new()),
            accumulations: Mutex::new(HashMap::new()),
            records: Mutex::new(HashMap::new()),
            actions: Mutex::new(HashMap::new()),
        }))
    }

    // 开启一个request
    fn begin_request(&self, id: &str, key: &str) {
        let full_id = format!("{}_{}", id, key);
        let mut pending_reqs = self.pending_reqs.lock().unwrap();
        match pending_reqs.entry(full_id) {
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
    fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        let mut pending_reqs = self.pending_reqs.lock().unwrap();
        let now = bucky_time_now();
        let full_id = format!("{}_{}", id, key);
        match pending_reqs.remove(&full_id) {
            Some(tick) => {
                let during = if now > tick {
                    now - tick
                } else {
                    0
                };

                let mut requests = self.requests.lock().unwrap();

                match requests.entry(id.to_owned()) {
                    Entry::Vacant(v) => {
                        let req = PerfRequestItem { 
                            time: bucky_time_now(), 
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

    fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        let mut accumulations = self.accumulations.lock().unwrap();

        match accumulations.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let acc = PerfAccumulationItem { 
                    time: bucky_time_now(), 
                    err, 
                    stat: size,
                };

                v.insert(vec![acc]);
            }
            Entry::Occupied(mut o) => {
                let item = o.get_mut();
                let acc = PerfAccumulationItem { 
                    time: bucky_time_now(), 
                    err, 
                    stat: size,
                };
                
                item.push(acc);

            }
        }
    }

    fn action(
        &self,
        id: &str,
        err: BuckyErrorCode,
        name: String,
        value: String,
    ){
        let mut actions = self.actions.lock().unwrap();

        match actions.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let action = PerfActionItem { 
                    time: bucky_time_now(), 
                    err, 
                    key: name.into(),
                    value: value.into(),
                };

                v.insert(vec![action]);
            }
            Entry::Occupied(mut o) => {
                let item = o.get_mut();
                let action = PerfActionItem { 
                    time: bucky_time_now(), 
                    err, 
                    key: name.into(),
                    value: value.into(),
                };
                
                item.push(action);

            }
        }
    }

    fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        let mut records = self.records.lock().unwrap();

        match records.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let record = PerfRecordItem { 
                    time: bucky_time_now(), 
                    total,
                    total_size,
                };
                v.insert(vec![record]);
            }
            Entry::Occupied(mut o) => {
                let v = o.get_mut();
                let record = PerfRecordItem { 
                    time: bucky_time_now(), 
                    total,
                    total_size,
                };
                v.push(record);

            }
        }
    }


}

#[derive(Clone)]
pub struct PerfIsolate(Arc<PerfIsolateInner>);

impl PerfIsolate {
    pub fn new(
        id: &str,
        version: String,
        span_time: u32,
        dec_id: Option<ObjectId>,
        perf_server_config: PerfServerConfig,
        stack: SharedCyfsStack,) -> Self {            
        let ret = PerfIsolateInner::new(
            id, version, span_time, dec_id,
            perf_server_config, stack);
            Self(Arc::new(ret))
    }

    pub async fn start(&self) {
        // 开启定期合并到store并保存
        let this = self.clone();
        async_std::task::spawn(async move {
            let _ = this.0.run_save().await;
        });

    }


}

impl Perf for PerfIsolate {

    fn get_id(&self) -> String {
        self.0.isolate_id.clone()
    }
    // create a new perf module
    fn fork(&self, id: &str) -> BuckyResult<Box<dyn Perf>> {
       self.0.fork(id)
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
