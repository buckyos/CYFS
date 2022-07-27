use crate::config::*;
use crate::isolate::PerfIsolate;
use async_std::task::block_on;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use crate::store::PerfStore;
use std::collections::{HashMap, hash_map};
use std::ops::Deref;
use std::sync::Arc;

pub struct PerfClientInner {
    id: String,
    version: String,

    perf_server_config: PerfServerConfig,

    store: PerfStore,

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
    
        let store = PerfStore::new(span_time, people_id, device_id, dec_id, id.to_owned(), stack);

        Self {
            id,
            version,
            perf_server_config,
            store,
            isolates: Mutex::new(HashMap::new()),
        }
    }

    pub fn new_isolate(&self, id: &str) -> PerfIsolate {
        let mut isolates = self.isolates.lock().unwrap();
        match isolates.entry(id.to_owned()) {
            hash_map::Entry::Vacant(v) => {
                log::info!("new isolate module: id={}", id);

                let isolate = PerfIsolate::new(id);
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

    // 开启定期保存的任务
    async fn run(&self) {
        loop {
            {
                let isolates = self.isolates.lock().unwrap();
                self.store.save(&isolates);
            }
            async_std::task::sleep(std::time::Duration::from_secs(5)).await;
        }
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

    pub async fn start(&self) -> BuckyResult<()> {
        // 开启定期合并到store并保存
        let this = self.clone();
        async_std::task::spawn(async move {
            this.0.run().await;
        });

        Ok(())
    }
}


impl Deref for PerfClient {
    type Target = PerfClientInner;
    fn deref(&self) -> &PerfClientInner {
        &self.0
    }
}
