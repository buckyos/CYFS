mod cache;
mod sqlite;

#[cfg(test)]
mod test;


use cache::*;
use sqlite::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub struct NamedObjectRelationCacheManager;

impl NamedObjectRelationCacheManager {
    pub async fn create(isolate: &str) -> BuckyResult<NamedObjectRelationCacheRef> {
        let storage = SqliteDBObjectRelationCache::new(isolate)?;
        let storage = Arc::new(Box::new(storage) as Box<dyn NamedObjectRelationCache>);
        
        // FIXME Use cyfs-stack's global-config for memory cache config
        let cache = NamedObjectRelationCacheMemoryCache::new(storage, 60 * 10, 1024, 1024);
        let cache = Arc::new(Box::new(cache) as Box<dyn NamedObjectRelationCache>);

        Ok(cache)
    }
}
