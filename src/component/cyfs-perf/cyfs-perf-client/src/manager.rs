use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use async_std::prelude::StreamExt;
use cyfs_base::{BuckyResult, NamedObject, ObjectId, OwnerObjectDesc};
use cyfs_lib::SharedCyfsStack;
use crate::PerfIsolate;
use crate::store::PerfStore;

pub struct IsolateManager {
    isolates: RwLock<HashMap<String, PerfIsolate>>,
    base_path: String,
    stack: SharedCyfsStack,
    dec_id: ObjectId,
}

pub type IsolateManagerRef = Arc<IsolateManager>;

impl IsolateManager {
    pub fn new(stack: SharedCyfsStack, dec_id: ObjectId) -> IsolateManagerRef {
        let ret = Self {
            isolates: RwLock::new(HashMap::new()),
            base_path: "".to_string(),
            stack,
            dec_id
        };

        // 启动save timer
        let manager_ref = Arc::new(ret);
        async_std::task::spawn(async move {
            // 每30分钟存一次
            let mut interval = async_std::stream::interval(Duration::from_secs(60*30));
            while let Some(_) = interval.next().await {
                let _ = manager_ref.inner_save().await;
            }
        });

        return manager_ref.clone();
    }

    pub fn fork(&self, id: &str, parent: &PerfIsolate) -> BuckyResult<PerfIsolate> {
        let mut lock = self.isolates.write()?;
        let ret = lock.entry(id.to_owned()).or_insert(parent.fork_self(id));
        return Ok(ret.clone());
    }

    async fn inner_save(&self) {
        if let Ok(mut lock) = self.isolates.read() {
            // 在这里lock一次/local/<dec_id>
            // 把/local/<dec_id>整个加载到op env或者state storage

            for (_id, iso) in lock.iter() {
                // 在这里内部操作op env或者state storage，不commit
                // save内部可以put noc，也可以把新对象返回给这里
                iso.save().await
            }

            // 如果把要save的新对象返回给这里，那么在这里统一put noc。现在可以先用for循环put，以后可能会有批量put的接口，效率会更高

            // 在这里commit一次
            // unlock /local/<dec_id>
        }
    }
}