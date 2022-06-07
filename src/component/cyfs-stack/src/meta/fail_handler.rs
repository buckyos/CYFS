use crate::meta::*;
use crate::resolver::DeviceCache;
use cyfs_base::*;

use cyfs_debug::Mutex;
use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;

const FAIL_CHECK_INTERVAL: u64 = 1000 * 1000 * 60 * 60;

struct DeviceFailHandlerImpl {
    meta_cache: Box<dyn MetaCache>,
    device_manager: Box<dyn DeviceCache>,

    // FIXME limit max cache count
    state: Mutex<HashMap<ObjectId, u64>>,
}

impl DeviceFailHandlerImpl {
    pub fn new(meta_cache: Box<dyn MetaCache>, device_manager: Box<dyn DeviceCache>) -> Self {
        Self {
            meta_cache,
            device_manager,
            state: Mutex::new(HashMap::new()),
        }
    }

    fn on_fail(&self, object_id: &ObjectId) -> bool {
        {
            let now = bucky_time_now();
            let mut state = self.state.lock().unwrap();
            match state.entry(object_id.to_owned()) {
                Entry::Occupied(mut v) => {
                    if now - v.get() < FAIL_CHECK_INTERVAL {
                        return false;
                    }

                    v.insert(now);
                }
                Entry::Vacant(v) => {
                    v.insert(now);
                }
            }
        }

        true
    }

    async fn flush_device(&self, device_id: &DeviceId) -> BuckyResult<()> {
        let ret = self.meta_cache.flush_object(device_id.object_id()).await?;
        if ret {
            info!("flush device and changed! device={}", device_id);
        } else {
            debug!("flush device and unchanged! device={}", device_id);
        }

        Ok(())
    }

    async fn flush_device_owner(&self, device_id: &DeviceId) -> BuckyResult<()> {
        let device = self.device_manager.search(device_id).await.map_err(|e| {
            error!("get target device failed! device={}, {}", device_id, e);
            e
        })?;

        // TODO 这里是否要支持多级owner，支持递归刷新？
        if let Some(owner) = device.desc().owner() {
            let ret = self.meta_cache.flush_object(owner).await?;
            if ret {
                info!(
                    "flush device's owner and changed! device={}, owner={}",
                    device_id, owner
                );
            } else {
                debug!(
                    "flush device's owner and unchanged! device={}, owner={}",
                    device_id, owner
                );
            }
        } else {
            warn!("device had now owner: device={}", device_id);
        }

        Ok(())
    }

    async fn try_flush_object(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        info!("will try flush object from meta: {}", object_id);

        match self.meta_cache.flush_object(&object_id).await {
            Ok(ret) => {
                if ret {
                    info!("object udpated: {}", object_id);
                    Ok(true)
                } else {
                    info!("flush object but not updated: {}", object_id);
                    Ok(false)
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[derive(Clone)]
pub struct ObjectFailHandler(Arc<DeviceFailHandlerImpl>);

impl ObjectFailHandler {
    pub fn new(meta_cache: Box<dyn MetaCache>, device_manager: Box<dyn DeviceCache>) -> Self {
        Self(Arc::new(DeviceFailHandlerImpl::new(
            meta_cache,
            device_manager,
        )))
    }

    pub fn on_device_fail(&self, device_id: &DeviceId) {
        if !self.0.on_fail(device_id.object_id()) {
            return;
        }

        {
            let handler = self.0.clone();
            let device_id = device_id.clone();
            async_std::task::spawn(async move {
                let _ = handler.flush_device(&device_id).await;
            });
        }

        {
            let handler = self.0.clone();
            let device_id = device_id.clone();
            async_std::task::spawn(async move {
                let _ = handler.flush_device_owner(&device_id).await;
            });
        }
    }

    // 如果获取到一个对象状态不对，那么尝试从meta上更新一下
    pub async fn try_flush_object(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        if !self.0.on_fail(object_id) {
            return Ok(false);
        }

        self.0.try_flush_object(object_id).await
    }
}
