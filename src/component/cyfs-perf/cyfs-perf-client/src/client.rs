use crate::config::*;
use crate::isolate::PerfIsolate;
use crate::reporter::*;
use crate::store::PerfStore;
use crate::noc_root_state::NocRootState;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use std::collections::{hash_map, HashMap};
use std::ops::Deref;
use std::sync::Arc;

// pub const PERF_DEC_ID_STR: &str = "5aSixgP8EPf6HkP54Qgybddhhsd1fgrkg7Atf2icJiiS";

pub struct PerfClientInner {
    id: String,
    version: String,
    dec_id: Option<ObjectId>,
    perf_server_config: PerfServerConfig,

    cyfs_stack: SharedCyfsStack,
    store: PerfStore,

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

        let dec_name = match &dec_id {
            Some(id) => id.to_string(),
            None => "system".to_owned(),
        };

        // 这里需要使用一个足够区分度的id，避免多个dec和内核共享协议栈情况下，同时使用PerfClient导致的冲突
        // TODO version区别对待和，不同版本的数据是否可以合并
        let store_object_id = format!("{}-{}-{}-cyfs-perf-store", device_id, dec_name, id);

        let store = PerfStore::new(store_object_id, &stack);

        Self {
            id,
            version,
            dec_id,
            perf_server_config,
            cyfs_stack: stack,
            store,
            isolates: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn start(&self) -> BuckyResult<()> {
        let req = UtilGetZoneOutputRequest::new(None, None);
        let resp = self.cyfs_stack.util().get_zone(req).await?;
        let people_id = resp.zone.owner().to_owned();

        if let Err(e) = self.store.start().await {
            // FIXME 启动失败如何处理？一般只有从noc加载失败才会出问题
            error!("perf client start error! {}", e);
        }

        let perf_server = PerfServerLoader::load_perf_server(self.perf_server_config.clone()).await;
        // route handler report to specified target perf server
        let reporter = PerfReporter::new(
            self.id.clone(),
            self.version.clone(),
            resp.device_id.clone(),
            people_id,
            self.dec_id.clone(),
            perf_server.clone(),
            self.cyfs_stack.clone(),
            self.store.clone(),
        );

        reporter.start();

        // noc root state update
        let noc_root_state = NocRootState::new(
            self.id.clone(),
            self.version.clone(),
            resp.device_id,
            people_id,
            self.dec_id.clone(),
            perf_server,
            self.cyfs_stack.clone(),
            self.store.clone(),
        );

        noc_root_state.start(); 

        Ok(())
    }

    // 开启定期保存的任务
    async fn run_save(&self) {
        loop {
            {
                let isolates = self.isolates.lock().unwrap();
                self.store.save(&isolates);
            }
            async_std::task::sleep(std::time::Duration::from_secs(60 * 2)).await;
        }
    }

    pub async fn flush(&self) -> BuckyResult<()> {
        // FIXME 如果刚好处于上报锁定期如何处理? 等待一段时间重试?

        let mut loop_count: i32 = 10;
        let mut dirty = false;
        loop {
            if self.store.is_locked() {
                loop_count -= 1;
                if loop_count <= 0 {
                    break;
                }

                async_std::task::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }

            let isolates = self.isolates.lock().unwrap();
            if isolates.is_empty() {
                break;
            }

            self.store.save(&isolates);
            dirty = true;
            break;
        }

        if dirty {
            self.store.flush().await?;
        }
        Ok(())
    }

    pub fn is_isolates_exists(&self, id: &str) -> bool {
        self.isolates.lock().unwrap().contains_key(id)
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

    pub async fn start(&self) -> BuckyResult<()> {
        self.0.start().await?;

        // 开启定期合并到store并保存
        let this = self.clone();
        async_std::task::spawn(async move {
            this.0.run_save().await;
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
