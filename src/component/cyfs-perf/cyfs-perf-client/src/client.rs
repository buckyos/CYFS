use crate::config::*;
use crate::isolate::PerfIsolate;
use async_std::task::block_on;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use std::collections::{hash_map, HashMap};
use std::ops::Deref;
use std::sync::Arc;

pub struct PerfClientInner {
    id: String,
    version: String,
    span_times: Vec<u32>,
    dec_id: Option<ObjectId>,
    perf_server_config: PerfServerConfig,

    people_id: ObjectId,
    device_id: ObjectId,

    cyfs_stack: SharedCyfsStack,

    isolates: Mutex<HashMap<String, PerfIsolate>>,
}

impl PerfClientInner {
    pub(crate) fn new(
        id: String,
        version: String,
        span_time: u32,
        dec_id: Option<ObjectId>,
        perf_server_config: PerfServerConfig,
        stack: SharedCyfsStack,
    ) -> Self {
        // let device_id = stack.local_device_id();
        let device_id = stack.local_device().desc().calculate_id();

        let req = UtilGetZoneOutputRequest::new(None, None);
        let resp = block_on(stack.util().get_zone(req)).unwrap();
        let people_id = resp.zone.owner().to_owned();
        
        let mut span_duration = span_time;
        if span_time < 1 || span_time >= 1440 {
            span_duration = 60;
        }
        let mut span_times = Vec::new();
        let mut seg = 0;
        while seg < 1440 {
            span_times.push(seg);
            seg += span_duration;
        }

        Self {
            id,
            version,
            span_times,
            dec_id,
            perf_server_config,
            people_id,
            device_id,
            cyfs_stack: stack,
            isolates: Mutex::new(HashMap::new()),
        }
    }

    pub fn is_isolates_exists(&self, id: &str) -> bool {
        self.isolates.lock().unwrap().contains_key(id)
    }

    pub fn new_isolate(&self, id: &str) -> PerfIsolate {
        let mut isolates = self.isolates.lock().unwrap();
        match isolates.entry(id.to_owned()) {
            hash_map::Entry::Vacant(v) => {
                log::info!("new isolate module: id={}", id);

                let isolate = PerfIsolate::new(id, &self.span_times, &self.people_id, &self.device_id, self.dec_id.clone(), &self.id, self.cyfs_stack.clone());
                let temp_isolate = isolate.clone();
                v.insert(isolate);
                temp_isolate.clone()
            }
            hash_map::Entry::Occupied(o) => {
                let msg = format!("isolate module already exists: id={}", id);
                log::error!("{}", msg);

                o.get().clone()
            }
        }
    }

    pub fn get_isolate(&self, id: &str) -> Option<PerfIsolate> {
        self.isolates.lock().unwrap().get(id).map(|v| v.clone())
    }
}

#[derive(Clone)]
pub struct PerfClient(Arc<PerfClientInner>);

impl PerfClient {
    pub fn new(
        id: String,
        version: String,
        span_time: u32,
        dec_id: Option<ObjectId>,
        perf_server_config: PerfServerConfig,
        stack: SharedCyfsStack,
    ) -> Self {
        let ret = PerfClientInner::new(id, version, span_time, dec_id, perf_server_config, stack);
        Self(Arc::new(ret))
    }
}


impl Deref for PerfClient {
    type Target = PerfClientInner;
    fn deref(&self) -> &PerfClientInner {
        &self.0
    }
}
