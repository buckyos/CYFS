use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use async_std::prelude::StreamExt;
use cyfs_base::{BuckyResult, NamedObject, ObjectId, ObjectMapSimpleContentType, OwnerObjectDesc};
use cyfs_lib::{GlobalStateCategory, SharedCyfsStack, StateStorageMap};
use cyfs_perf_base::PERF_SERVICE_DEC_ID;
use crate::PerfIsolate;
use crate::store::PerfStore;

pub struct IsolateManager {
    isolates: RwLock<HashMap<String, PerfIsolate>>,
}

pub type IsolateManagerRef = Arc<IsolateManager>;

impl IsolateManager {
    pub fn new(stack: SharedCyfsStack, dec_id: ObjectId, span_time: u32) -> IsolateManagerRef {
        let ret = Self {
            isolates: RwLock::new(HashMap::new()),
        };

        let manager_ref = Arc::new(ret);

        let manager = manager_ref.clone();
        async_std::task::spawn(async move {
            let perf_dec_id = ObjectId::from_str(PERF_SERVICE_DEC_ID).unwrap();
            let path = format!("/local/{}", dec_id.to_string());
            let storage = stack.global_state_storage_ex(
                GlobalStateCategory::LocalCache,
                path,
                ObjectMapSimpleContentType::Map,
                None,
                Some(perf_dec_id),
            );

            if let Err(e) = storage.init().await {
                error!("state storage initialized failed: {:?}", e);
                return;
            }

            let map = StateStorageMap::new(storage);

            // 这里拿到people id 是作为后续创建对象用的
            let device_id = stack.local_device_id().object_id().clone();
            let people_id = stack.local_device().desc().owner().unwrap_or(device_id);

            let store = PerfStore::new(span_time, people_id, dec_id, stack);

            // 启动save timer
            // 每30分钟存一次
            let mut interval = async_std::stream::interval(Duration::from_secs(10));
            while let Some(_) = interval.next().await {
                let _ = manager.inner_save(&store, &map).await;
            }
        });

        return manager_ref;
    }

    pub fn fork(&self, id: &str, parent: &PerfIsolate) -> Option<PerfIsolate> {
        if let Ok( mut lock) = self.isolates.write() {
            let ret = lock.entry(id.to_owned()).or_insert(parent.fork_self(id));
            return Some(ret.clone());
        }

        return None;
    }
    
    async fn inner_save(&self, store: &PerfStore, map: &StateStorageMap) -> BuckyResult<()> {
        let mut items = vec![];
        if let Ok(lock) = self.isolates.read() {
            for (_id, iso) in lock.iter() {
                // 数据弹出, 不能在rwlock里用await
                items.push(iso.take_data());
            }
        }

        // 在这里内部操作op env或者state storage，不commit
        // save内部可以put noc，也可以把新对象返回给这里
        for item in items {
            // FIXME:  futures::future::join_all parallel 
            store.request(map, &item.isolate_id, item.requests).await?;
            store.acc(map, &item.isolate_id, item.accumulations).await?;
            store.action(map, &item.isolate_id, item.actions).await?;
            store.record(map, &item.isolate_id, item.records).await?;
        }

        let _ = map.storage().save().await;

        Ok(())
    }
}