use crate::*;

use async_std::sync::Mutex as AsyncMutex;
use std::future::Future;

use std::sync::{Arc, RwLock};

#[async_trait::async_trait]
pub trait ObjectMapRootEvent: Sync + Send + 'static {
    async fn root_updated(
        &self,
        dec_id: &Option<ObjectId>,
        new_root_id: ObjectId,
        prev_id: ObjectId,
    ) -> BuckyResult<()>;
}

pub type ObjectMapRootEventRef = Arc<Box<dyn ObjectMapRootEvent>>;

#[derive(Clone)]
pub struct ObjectMapRootHolder {
    dec_id: Option<ObjectId>,

    // 当前的读写锁，只有在持有update_lock情况下，才可以更新
    root: Arc<RwLock<ObjectId>>,
    update_lock: Arc<AsyncMutex<()>>,
    event: ObjectMapRootEventRef,
}

impl ObjectMapRootHolder {
    pub fn new(dec_id: Option<ObjectId>, root: ObjectId, event: ObjectMapRootEventRef) -> Self {
        Self {
            dec_id,
            root: Arc::new(RwLock::new(root)),
            update_lock: Arc::new(AsyncMutex::new(())),
            event,
        }
    }

    pub fn get_current_root(&self) -> ObjectId {
        self.root.read().unwrap().clone()
    }

    // direct set the root_state without notify event
    pub async fn direct_reload_root(&self, new_root_id: ObjectId) {
        let _update_lock = self.update_lock.lock().await;
        let mut current = self.root.write().unwrap();

        info!(
            "reload objectmap root holder's root! dec={:?}, current={}, new={}",
            self.dec_id, *current, new_root_id
        );
        *current = new_root_id;
    }

    // 尝试更新root，同一个root同一时刻只能有一个操作在进行，通过异步锁来保证
    pub async fn update_root<F, Fut>(&self, update_root_fn: F) -> BuckyResult<ObjectId>
    where
        F: FnOnce(ObjectId) -> Fut,
        Fut: Future<Output = BuckyResult<ObjectId>>,
    {
        let _update_lock = self.update_lock.lock().await;
        let root = self.get_current_root();
        let new_root = update_root_fn(root.clone()).await?;
        if new_root != root {
            info!("will update root holder: {} -> {}", root, new_root);

            // 必须先触发事件，通知上层更新全局状态
            if let Err(e) = self
                .event
                .root_updated(&self.dec_id, new_root.clone(), root.clone())
                .await
            {
                error!(
                    "root update event notify error! {} -> {}, {}",
                    root, new_root, e
                );

                return Err(e);
            }

            // 触发事件成功后，才可以更新root-holder
            // 避免这两个操作之间，新的root-holder被使用但全局根状态由于没更新导致的各种异常
            {
                let mut current = self.root.write().unwrap();
                assert_eq!(*current, root);
                *current = new_root.clone();
            }

            info!("root updated! {} -> {}", root, new_root);
        }

        Ok(new_root)
    }
}