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
    dec_id: Option<ObjectId>,
    perf_server_config: PerfServerConfig,

    people_id: ObjectId,
    device_id: DeviceId,

    cyfs_stack: SharedCyfsStack,

    isolates: Mutex<HashMap<String, PerfIsolate>>,
}

impl PerfClientInner {
    pub(crate) fn new(
        id: String,
        version: String,
        dec_id: Option<ObjectId>,
        perf_server_config: PerfServerConfig,
        stack: SharedCyfsStack,
    ) -> Self {
        let device_id = stack.local_device_id();

        // let dec_name = match &dec_id {
        //     Some(id) => id.to_string(),
        //     None => "system".to_owned(),
        // };

        let req = UtilGetZoneOutputRequest::new(None, None);
        let resp = block_on(stack.util().get_zone(req)).unwrap();
        let people_id = resp.zone.owner().to_owned();

        Self {
            id,
            version,
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

                let isolate = PerfIsolate::new(id, self.people_id.clone(), self.dec_id.clone(), self.id.to_owned(), self.cyfs_stack.clone());
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
        dec_id: Option<ObjectId>,
        perf_server_config: PerfServerConfig,
        stack: SharedCyfsStack,
    ) -> Self {
        let ret = PerfClientInner::new(id, version, dec_id, perf_server_config, stack);
        Self(Arc::new(ret))
    }
}


impl Deref for PerfClient {
    type Target = PerfClientInner;
    fn deref(&self) -> &PerfClientInner {
        &self.0
    }
}
