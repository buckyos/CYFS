use crate::sqlite::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct DataCacheManager;

impl DataCacheManager {
    pub fn create_data_cache(isolate: &str) -> BuckyResult<Box<dyn NamedDataCache>> {
        let cache = SqliteDBDataCache::new(isolate)?;
        Ok(Box::new(cache))
    }
}
