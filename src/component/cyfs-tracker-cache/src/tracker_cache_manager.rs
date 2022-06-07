use crate::sqlite::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct TrackerCacheManager {}

impl TrackerCacheManager {
    pub fn create_tracker_cache(isolate: &str) -> BuckyResult<Box<dyn TrackerCache>> {
        let cache = SqliteDBDataCache::new(isolate)?;
        Ok(Box::new(cache))
    }
}
