use crate::cache::*;
use crate::prelude::*;
use crate::storage::*;
use cyfs_base::*;

use std::sync::Arc;

pub struct NamedObjectCacheManager;


impl NamedObjectCacheManager {
    pub async fn create(isolate: &str) -> BuckyResult<NamedObjectCacheRef> {
        let storage_raw = NamedObjectLocalStorage::new(isolate).await?;
        let storage_raw = Arc::new(Box::new(storage_raw) as Box<dyn NamedObjectCache1>);
        let storage = NamedObjectCacheSerializer::new(storage_raw);

        let storage = Arc::new(Box::new(storage) as Box<dyn NamedObjectCache1>);


        // FIXME Use cyfs-stack's global-config for memory cache config
        let cache = NamedObjectCacheMemoryCache::new(storage, 60 * 10, 1024);
        let cache = Arc::new(Box::new(cache) as Box<dyn NamedObjectCache1>);

        Ok(cache)
    }
}
