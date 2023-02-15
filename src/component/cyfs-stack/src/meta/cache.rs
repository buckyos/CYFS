use cyfs_base::*;
use cyfs_debug::Mutex;

use lru_time_cache::LruCache;
use std::borrow::Borrow;
use std::sync::Arc;

#[derive(Clone)]
struct MetaCacheItem<Value> {
    last_update_time: u64,
    value: Value,
}

struct MetaMemoryCacheImpl<Key, Value>
where
    Key: Ord + Clone,
{
    list: LruCache<Key, MetaCacheItem<Value>>,
}

impl<Key, Value> MetaMemoryCacheImpl<Key, Value>
where
    Key: Ord + Clone,
    Value: Clone,
{
    pub fn new(timeout_in_secs: u64) -> Self {
        let list = LruCache::with_expiry_duration_and_capacity(
            std::time::Duration::from_secs(timeout_in_secs),
            256,
        );

        Self { list }
    }

    pub fn add(&mut self, key: Key, value: Value) {
        let item = MetaCacheItem {
            value,
            last_update_time: bucky_time_now(),
        };

        self.list.insert(key, item);
    }

    pub fn get<Q: ?Sized>(&mut self, key: &Q) -> Option<Value>
    where
        Key: Borrow<Q>,
        Q: Ord,
    {
        // force remove expired items 
        self.list.iter();

        // do not update the last used timestamp
        self.list.peek(&key).map(|v| v.value.clone())
    }
}

#[derive(Clone)]
pub(crate) struct MetaMemoryCache<Key, Value>(Arc<Mutex<MetaMemoryCacheImpl<Key, Value>>>)
where
    Key: Ord + Clone;

impl<Key, Value> MetaMemoryCache<Key, Value>
where
    Key: Ord + Clone,
    Value: Clone,
{
    pub fn new(timeout_in_secs: u64) -> Self {
        Self(Arc::new(Mutex::new(MetaMemoryCacheImpl::new(
            timeout_in_secs,
        ))))
    }

    pub fn add(&self, key: Key, value: Value) {
        self.0.lock().unwrap().add(key, value)
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<Value>
    where
        Key: Borrow<Q>,
        Q: Ord,
    {
        self.0.lock().unwrap().get(key)
    }
}

pub(crate) type MetaMemoryCacheForObject = MetaMemoryCache<ObjectId, Vec<u8>>;
pub(crate) type MetaMemoryCacheForName = MetaMemoryCache<String, Option<(NameInfo, NameState)>>;