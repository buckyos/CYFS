use crate::meta::*;
use crate::resolver::DeviceCache;
use crate::zone::*;
use cyfs_base::*;

use cyfs_debug::Mutex;
use lru_time_cache::LruCache;
use std::sync::Arc;

const FAIL_CHECK_INTERVAL: u64 = 1000 * 1000 * 60 * 60;

struct DeviceFailHandlerImpl {
    meta_cache: MetaCacheRef,
    device_manager: Box<dyn DeviceCache>,
    zone_manager: once_cell::sync::OnceCell<ZoneManagerRef>,

    state: Mutex<LruCache<ObjectId, ()>>,
}

impl DeviceFailHandlerImpl {
    pub fn new(meta_cache: MetaCacheRef, device_manager: Box<dyn DeviceCache>) -> Self {
        Self {
            meta_cache,
            device_manager,
            zone_manager: once_cell::sync::OnceCell::new(),

            state: Mutex::new(LruCache::with_expiry_duration_and_capacity(
                std::time::Duration::from_secs(60 * 30),
                1024,
            )),
        }
    }

    fn bind_zone_manager(&self, zone_manager: ZoneManagerRef) {
        if let Err(_) = self.zone_manager.set(zone_manager) {
            unreachable!();
        }
    }

    fn on_fail(&self, object_id: &ObjectId) -> bool {
        let mut state = self.state.lock().unwrap();

        // remove expired objects
        state.iter();

        match state.peek(object_id) {
            Some(_) => false,
            None => {
                state.insert(object_id.to_owned(), ());
                true
            }
        }
    }

    async fn flush_device(&self, device_id: &DeviceId) -> BuckyResult<bool> {
        let ret = self.meta_cache.flush_object(device_id.object_id()).await?;
        if ret {
            info!("flush device and changed! device={}", device_id);
            self.device_manager.flush(device_id).await;
        } else {
            info!("flush device and unchanged! device={}", device_id);
        }

        Ok(ret)
    }

    async fn flush_device_owner(&self, device_id: &DeviceId) -> BuckyResult<()> {
        let device = self.device_manager.search(device_id).await.map_err(|e| {
            error!("get target device failed! device={}, {}", device_id, e);
            e
        })?;

        // TODO Need support multi-level OWNER here, support recursive refresh?
        if let Some(owner) = device.desc().owner() {
            let ret = self.meta_cache.flush_object(owner).await?;
            if ret {
                info!(
                    "flush device's owner and changed! device={}, owner={}",
                    device_id, owner
                );

                // try reflush current zone's info
                if let Some(zone_manager) = self.zone_manager.get() {
                    zone_manager.remove_zone_by_device(device_id).await;
                }
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
                    info!("object updated: {}", object_id);
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
    pub fn new(meta_cache: MetaCacheRef, device_manager: Box<dyn DeviceCache>) -> Self {
        Self(Arc::new(DeviceFailHandlerImpl::new(
            meta_cache,
            device_manager,
        )))
    }

    pub fn bind_zone_manager(&self, zone_manager: ZoneManagerRef) {
        self.0.bind_zone_manager(zone_manager)
    }

    pub async fn on_device_fail(&self, device_id: &DeviceId) -> BuckyResult<bool> {
        if !self.0.on_fail(device_id.object_id()) {
            return Ok(false);
        }

          
        let ret = self.0.flush_device(&device_id).await;
        
        {
            let handler = self.0.clone();
            let device_id = device_id.clone();
            async_std::task::spawn(async move {
                let _ = handler.flush_device_owner(&device_id).await;
            });
        }

        ret
    }

    // If the state is wrong, then try to flush object from Meta
    pub async fn try_flush_object(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        if !self.0.on_fail(object_id) {
            return Ok(false);
        }

        self.0.try_flush_object(object_id).await
    }
}
