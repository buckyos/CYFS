use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use async_std::prelude::StreamExt;
use cyfs_base::{BuckyResult, NamedObject, ObjectId, OwnerObjectDesc};
use cyfs_lib::{SharedCyfsStack, GlobalStateStub};
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
            // 这里拿到people id 是作为后续创建对象用的
            let device_id = stack.local_device_id().object_id().clone();
            let people_id = stack.local_device().desc().owner().unwrap_or(device_id);

            let perf_dec_id = ObjectId::from_str(PERF_SERVICE_DEC_ID).unwrap();
            let root_state = stack.root_state_stub(None, Some(perf_dec_id));
            let path = format!("/local/{}", dec_id.to_string());

            let store = PerfStore::new(span_time, people_id, dec_id, stack);

            // 启动save timer
            // 每30分钟存一次
            let mut interval = async_std::stream::interval(Duration::from_secs(10));
            while let Some(_) = interval.next().await {
                info!("waitings 10s ...");
                let _ = manager.inner_save(&store, path.to_owned(), &root_state).await;
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
    
    async fn inner_save(&self, store: &PerfStore, _path: String, root_state: &GlobalStateStub) -> BuckyResult<()> {
        let mut items = vec![];
        if let Ok(lock) = self.isolates.read() {
            for (_id, iso) in lock.iter() {
                // 数据弹出, 不能在rwlock里用await
                items.push(iso.take_data());
            }
        }
        // 在这里lock一次/local/<dec_id>
        // 把/local/<dec_id>整个加载到op env
        let op_env = root_state.create_path_op_env().await?;
        // lock 一直在自旋?
        // op_env.lock(vec![path.to_owned()], 0).await?;

        // // 如果把要save的新对象返回给这里，那么在这里统一put noc。现在可以先用for循环put，以后可能会有批量put的接口，效率会更高
        // let single_op_env = root_state.create_single_op_env().await?;
        // if let Err(e) = single_op_env.load_by_path(path.to_owned()).await {
        //     warn!("inner_save load_by_path error: {e}");
        // }

        // 在这里内部操作op env或者state storage，不commit
        // save内部可以put noc，也可以把新对象返回给这里
        for item in items {
            // FIXME:  futures::future::join_all parallel 
            store.request(&op_env, &item.isolate_id, item.requests).await?;
            store.acc(&op_env, &item.isolate_id, item.accumulations).await?;
            store.action(&op_env, &item.isolate_id, item.actions).await?;
            store.record(&op_env, &item.isolate_id, item.records).await?;
        }

        // 如果把要save的新对象返回给这里，那么在这里统一put noc。现在可以先用for循环put，以后可能会有批量put的接口，效率会更高

        // unlock /local/<dec_id>
        // 在这里commit一次
        let root = op_env.commit().await?;
        info!("new dec root is: {:?}", root);

        // single_op_env.abort().await?;        
        Ok(())
    }
}