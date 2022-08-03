use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use async_std::prelude::StreamExt;
use cyfs_base::{BuckyResult, NamedObject, ObjectId, OwnerObjectDesc};
use cyfs_lib::{SharedCyfsStack, GlobalStateStub};
use cyfs_perf_base::PERF_SERVICE_DEC_ID;
use crate::PerfIsolate;
use crate::store::PerfStore;

pub struct IsolateManager {
    isolates: RwLock<HashMap<String, PerfIsolate>>,
    // benchmark
    stack: SharedCyfsStack,
    dec_id: ObjectId,
    span_time: u32,
}

pub type IsolateManagerRef = Arc<IsolateManager>;

impl IsolateManager {
    pub fn new(stack: SharedCyfsStack, dec_id: ObjectId, span_time: u32) -> IsolateManagerRef {
        let ret = Self {
            isolates: RwLock::new(HashMap::new()),
            stack: stack.clone(),
            dec_id: dec_id.clone(),
            span_time,
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
            let mut interval = async_std::stream::interval(Duration::from_secs(60 * 30));
            while let Some(_) = interval.next().await {
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

    pub async fn save_test(&self) {
        // 这里拿到people id 是作为后续创建对象用的
        let device_id = self.stack.local_device_id().object_id().clone();
        let people_id = self.stack.local_device().desc().owner().unwrap_or(device_id);
        
        let root_state = self.stack.root_state_stub(None, Some(ObjectId::default()));

        let store = PerfStore::new(self.span_time, people_id, self.dec_id, self.stack.clone());

        let mut items = vec![];
        if let Ok(lock) = self.isolates.read() {
            for (_id, iso) in lock.iter() {
                // 数据弹出, 不能在rwlock里用await
                items.push(iso.take_data());
            }
        }

        println!("items: {}", items.len());

        let op_env = root_state.create_path_op_env().await.unwrap();

        for item in items {
            // FIXME:  futures::future::join_all parallel 
            store.request(&op_env, &item.isolate_id, item.requests).await.unwrap();
            store.acc(&op_env, &item.isolate_id, item.accumulations).await.unwrap();
            store.action(&op_env, &item.isolate_id, item.actions).await.unwrap();
            store.record(&op_env, &item.isolate_id, item.records).await.unwrap();
        }

        let _root = op_env.commit().await.unwrap();

        println!("case done...");
    }
    
    async fn inner_save(&self, store: &PerfStore, path: String, root_state: &GlobalStateStub) -> BuckyResult<()> {
        let start = Instant::now();

        // 在这里lock一次/local/<dec_id>
        // 把/local/<dec_id>整个加载到op env
        let op_env = root_state.create_path_op_env().await.unwrap();
        // lock 一直在自旋?
        op_env.try_lock(vec![path.to_owned()], 0).await?;

        let mut items = vec![];
        if let Ok(lock) = self.isolates.read() {
            for (_id, iso) in lock.iter() {
                // 数据弹出, 不能在rwlock里用await
                items.push(iso.take_data());
            }
        }

        if items.is_empty() {
            return Ok(());
        }

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

        info!("batch commit cost miliseconds: {}", start.elapsed().as_millis());
        info!("new dec root is: {:?}", root);

        // single_op_env.abort().await?;        
        Ok(())
    }
}