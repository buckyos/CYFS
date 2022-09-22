use crate::cache::*;
use crate::storage::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub struct NamedObjectCacheManager;

impl NamedObjectCacheManager {
    pub async fn create(isolate: &str) -> BuckyResult<NamedObjectCacheRef> {
        let storage_raw = NamedObjectLocalStorage::new(isolate).await?;
        let meta = storage_raw.meta().clone();
        let storage_raw = Arc::new(Box::new(storage_raw) as Box<dyn NamedObjectCache>);
        
        // FIXME Use cyfs-stack's global-config for memory cache config
        let cache = NamedObjectCacheMemoryCache::new(meta, storage_raw, 60 * 10, 1024);
        let cache = Arc::new(Box::new(cache) as Box<dyn NamedObjectCache>);

        let serial_cache = NamedObjectCacheSerializer::new(cache);
        let serial_cache = Arc::new(Box::new(serial_cache) as Box<dyn NamedObjectCache>);

        Ok(serial_cache)
    }
}
