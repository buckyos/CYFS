use super::isolate::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_perf_base::*;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// 基于noc的统计项缓存
// 需要注意数据丢失和数据重复的两个核心问题，需要小心处理

#[derive(Clone)]
pub(crate) struct PerfStore {
    cache: NOCCollectionSync<PerfIsolateEntityList>,

    // 上报时候，需要锁定，避免出现数据不一致问题
    locked: Arc<AtomicBool>,
}

impl PerfStore {
    pub fn new(id: String, stack: &UniCyfsStackRef, device_id: &DeviceId) -> Self {
        let cache = RemoteNOCStorage::new_noc_collection_sync_uni(&id, stack, device_id);
        let locked = Arc::new(AtomicBool::new(false));
        Self { cache, locked }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        // 首先从noc里面加载已经缓存的数据
        if let Err(e) = self.cache.load().await {
            if e.code() != BuckyErrorCode::InvalidFormat {
                error!("load perf cache from noc error: {}", e);
                return Err(e);
            }

            // 如果加载到了不可识别的数据，说明版本出问题了，这里直接重置整个对象
            warn!(
                "load perf cache from noc but unrecognizable data! now will clear all {}",
                e
            );
            self.cache.set_dirty(true);
            self.cache.async_save();
        }

        debug!(
            "load perf data from noc: id={}, {:?}",
            self.cache.id(),
            self.cache.coll().lock().unwrap()
        );

        // 开启定时保存(到noc)
        self.cache.start_save(std::time::Duration::from_secs(60));

        Ok(())
    }

    pub async fn flush(&self) -> BuckyResult<()> {
        self.cache.save().await
    }

    // 尝试保存到noc，保存成功后会清空isolates内容
    pub fn save(&self, isolates: &HashMap<String, PerfIsolate>) {
        // 锁定状态下，不可修改数据
        if self.is_locked() {
            warn!("perf store still in locked state!");
            return;
        }

        let mut dirty = false;
        for (key, isolate) in isolates {
            let data = isolate.take_data();
            if data.is_empty() {
                continue;
            }

            info!("will save perf isolate: {}, data={:?}", key, data);
            self.cache.coll().lock().unwrap().merge(data);
            dirty = true;
        }

        if !dirty {
            // 如果这个周期没产生任何统计数据，那么不需要触发合并和保存了
            return;
        }

        self.cache.set_dirty(true);
    }

    // 拷贝一份数据用以上报
    pub fn clone_data(&self) -> PerfIsolateEntityList {
        self.cache.coll().lock().unwrap().clone()
    }

    // 上报成功后，清除本地数据，避免重复上报
    pub fn clear_data(&self) {
        self.cache.coll().lock().unwrap().clear();

        // 清除数据后，立即保存一次
        self.cache.set_dirty(true);
        self.cache.async_save();
    }

    // 锁定区间用以上报操作
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }

    pub fn lock_for_report(&self) -> bool {
        let ret = self.locked.swap(true, Ordering::SeqCst);
        if !ret {
            info!("lock perf store for reporting!");
        } else {
            error!("perf store already been locked!");
        }

        ret
    }

    pub fn unlock_for_report(&self) {
        let ret = self.locked.swap(false, Ordering::SeqCst);
        if ret {
            info!("unlock perf store after report!");
        } else {
            error!("perf store not been locked yet!");
        }
    }
}
