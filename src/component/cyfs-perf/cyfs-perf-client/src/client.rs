use crate::config::*;
use crate::isolate::PerfIsolateInstance;
use crate::reporter::*;
use crate::store::PerfStore;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use std::collections::{HashMap};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

// pub const PERF_DEC_ID_STR: &str = "5aSixgP8EPf6HkP54Qgybddhhsd1fgrkg7Atf2icJiiS";

pub struct PerfClientInner {
    id: String,
    version: String,
    dec_id: Option<ObjectId>,
    perf_config: PerfConfig,

    cyfs_stack: UniCyfsStackRef,
    store: PerfStore,

    isolates: Mutex<HashMap<String, PerfIsolateInstance>>,

    local_device: DeviceId,
    owner: ObjectId
}

pub struct PerfConfig {
    pub reporter: PerfServerConfig,
    pub save_to_file: bool,
    pub report_interval: Duration
}

impl PerfClientInner {
    pub(crate) fn new(
        id: String,
        version: String,
        dec_id: Option<ObjectId>,
        perf_config: PerfConfig,
        stack: UniCyfsStackRef,
        local_device: DeviceId,
        owner: ObjectId
    ) -> Self {
        let dec_name = match &dec_id {
            Some(id) => id.to_string(),
            None => "system".to_owned(),
        };

        // 这里需要使用一个足够区分度的id，避免多个dec和内核共享协议栈情况下，同时使用PerfClient导致的冲突
        // TODO version区别对待和，不同版本的数据是否可以合并
        let store_object_id = format!("{}-{}-{}-cyfs-perf-store", &local_device, dec_name, id);

        let store = PerfStore::new(store_object_id, &stack, &local_device);

        Self {
            id,
            version,
            dec_id,
            perf_config,
            cyfs_stack: stack,
            store,
            isolates: Mutex::new(HashMap::new()),
            local_device,
            owner
        }
    }

    pub(crate) async fn start(&self) -> BuckyResult<()> {
        if let Err(e) = self.store.start().await {
            // FIXME 启动失败如何处理？一般只有从noc加载失败才会出问题
            error!("perf client start error! {}", e);
        }

        let save_to_local = self.perf_config.reporter.is_none();
        let perf_server = PerfServerLoader::load_perf_server(self.perf_config.reporter.clone()).await;

        let reporter = PerfReporter::new(
            self.id.clone(),
            self.version.clone(),
            self.local_device.clone(),
            self.owner.clone(),
            self.dec_id.clone(),
            perf_server,
            self.cyfs_stack.clone(),
            self.store.clone(),
            save_to_local,
            self.perf_config.save_to_file,
            self.perf_config.report_interval,
        );

        reporter.start();

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

    pub fn get_isolate(&self, id: &str) -> PerfIsolateInstance {
        let mut isolates = self.isolates.lock().unwrap();
        isolates.entry(id.to_owned()).or_insert_with(|| {
            log::info!("create isolate module: id={}", id);
            PerfIsolateInstance::new(id)
        }).clone()
    }
}

#[derive(Clone)]
pub struct PerfClient(Arc<PerfClientInner>);

impl PerfClient {
    pub fn new(
        id: String,
        version: String,
        dec_id: Option<ObjectId>,
        perf_config: PerfConfig,
        stack: UniCyfsStackRef,
        local_device: DeviceId, owner: ObjectId
    ) -> Self {
        let ret = PerfClientInner::new(id, version, dec_id, perf_config, stack, local_device, owner);
        Self(Arc::new(ret))
    }

    pub fn get_isolate(&self, id: &str) -> PerfIsolateInstance {
        self.0.get_isolate(id)
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


#[async_trait::async_trait]
impl PerfManager for PerfClient {
    async fn flush(&self) -> BuckyResult<()> {
        self.0.flush().await
    }

    fn get_isolate(&self, id: &str) -> PerfIsolateRef {
        self.0.get_isolate(id).into_isolate()
    }
}