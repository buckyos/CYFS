use crate::front::FrontARequestVersion;
use cyfs_base::*;

use lru_time_cache::LruCache;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct AppVersionCacheKey {
    dec_id: ObjectId,
    version: FrontARequestVersion,
}

pub struct AppCacheInner {
    name: LruCache<String, (ObjectId, u64)>,
    name_not_exists: LruCache<String, u64>,

    version: LruCache<AppVersionCacheKey, (Option<ObjectId>, u64)>,
}

impl AppCacheInner {
    pub fn new() -> Self {
        Self {
            name: LruCache::with_expiry_duration_and_capacity(
                std::time::Duration::from_secs(3600 * 24),
                256,
            ),
            name_not_exists: LruCache::with_expiry_duration_and_capacity(
                std::time::Duration::from_secs(60 * 10),
                512,
            ),

            version: LruCache::with_expiry_duration_and_capacity(
                std::time::Duration::from_secs(60 * 10),
                256,
            ),
        }
    }

    fn get_app_by_name(&mut self, name: &str) -> Option<Option<ObjectId>> {
        let _ = self.name.iter();

        let ret = self.name.peek(name).cloned();
        if ret.is_some() {
            return Some(ret.map(|v| v.0));
        }

        let _ = self.name_not_exists.iter();

        let ret = self.name_not_exists.peek(name);
        if ret.is_some() {
            return Some(None);
        }

        None
    }

    fn cache_app_with_name(&mut self, name: &str, result: Option<ObjectId>) {
        match result {
            Some(dec_id) => {
                self.name
                    .insert(name.to_owned(), (dec_id, bucky_time_now()));
            }
            None => {
                self.name_not_exists
                    .insert(name.to_owned(), bucky_time_now());
            }
        }
    }

    fn get_dir_by_version(
        &mut self,
        dec_id: &ObjectId,
        version: &FrontARequestVersion,
    ) -> Option<Option<ObjectId>> {
        let _ = self.version.iter();

        let key = AppVersionCacheKey {
            dec_id: dec_id.to_owned(),
            version: version.to_owned(),
        };

        self.version.peek(&key).cloned().map(|v| v.0)
    }

    fn cache_dir_with_version(
        &mut self,
        dec_id: &ObjectId,
        version: &FrontARequestVersion,
        result: Option<ObjectId>,
    ) {
        let key = AppVersionCacheKey {
            dec_id: dec_id.to_owned(),
            version: version.to_owned(),
        };

        self.version.insert(key, (result, bucky_time_now()));
    }

    fn clear_dir(&mut self, dec_id: &ObjectId, ver: &FrontARequestVersion) {
        let key = AppVersionCacheKey {
            dec_id: dec_id.to_owned(),
            version: ver.to_owned(),
        };

        if let Some(ret) = self.version.peek(&key) {
            // In order to avoid quick refresh, a minimum refresh interval of 1 minute is reserved here
            if bucky_time_now() - ret.1 >= 1000 * 1000 * 60 {
                drop(ret);

                let ret = self.version.remove(&key).unwrap();
                info!(
                    "clear app dir cache: dec={}, ver={:?}, cache={:?}",
                    dec_id, ver, ret.1
                );
            }
        }
    }
}

#[derive(Clone)]
pub struct AppCache(Arc<Mutex<AppCacheInner>>);

impl AppCache {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(AppCacheInner::new())))
    }

    pub fn get_app_by_name(&self, name: &str) -> Option<Option<ObjectId>> {
        self.0.lock().unwrap().get_app_by_name(name)
    }

    pub fn cache_app_with_name(&self, name: &str, result: Option<ObjectId>) {
        self.0.lock().unwrap().cache_app_with_name(name, result)
    }

    pub fn get_dir_by_version(
        &self,
        dec_id: &ObjectId,
        version: &FrontARequestVersion,
    ) -> Option<Option<ObjectId>> {
        self.0.lock().unwrap().get_dir_by_version(dec_id, version)
    }

    pub fn cache_dir_with_version(
        &self,
        dec_id: &ObjectId,
        version: &FrontARequestVersion,
        result: Option<ObjectId>,
    ) {
        self.0
            .lock()
            .unwrap()
            .cache_dir_with_version(dec_id, version, result)
    }

    pub fn clear_dir(&self, dec_id: &ObjectId, ver: &FrontARequestVersion) {
        self.0.lock().unwrap().clear_dir(dec_id, ver)
    }
}
