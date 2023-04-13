use super::object_map::{ObjectMap, ObjectMapRef};
use super::visitor::*;
use crate::*;

use async_std::sync::Mutex as AsyncMutex;
use std::any::Any;
use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex};

/*
ObjectMap的缓存设计
每个op_env有独立的写缓存，用以暂存所有写入操作，在commit时候写入下层缓存
每个root共享一个读缓存
*/

#[derive(Clone)]
pub struct ObjectMapCacheItem {
    pub object: ObjectMap,
    pub access: AccessString,
}

// objectmap的依赖的noc接口，实现了object的最终保存和加载
#[async_trait::async_trait]
pub trait ObjectMapNOCCache: Send + Sync {
    async fn exists(&self, dec: Option<ObjectId>, object_id: &ObjectId) -> BuckyResult<bool>;

    async fn get_object_map(
        &self,
        dec: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMap>> {
        self.get_object_map_ex(dec, object_id)
            .await
            .map(|ret| ret.map(|v| v.object))
    }

    async fn get_object_map_ex(
        &self,
        dec: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapCacheItem>>;

    async fn put_object_map(
        &self,
        dec: Option<ObjectId>,
        object_id: ObjectId,
        object: ObjectMap,
        access: Option<AccessString>,
    ) -> BuckyResult<()>;
}

pub type ObjectMapNOCCacheRef = Arc<Box<dyn ObjectMapNOCCache>>;

// 简单的内存版本的noc
pub(crate) struct ObjectMapMemoryNOCCache {
    cache: Mutex<HashMap<ObjectId, ObjectMapCacheItem>>,
}

impl ObjectMapMemoryNOCCache {
    pub fn new() -> ObjectMapNOCCacheRef {
        let ret = Self {
            cache: Mutex::new(HashMap::new()),
        };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl ObjectMapNOCCache for ObjectMapMemoryNOCCache {
    async fn exists(&self, dec_id: Option<ObjectId>, object_id: &ObjectId) -> BuckyResult<bool> {
        info!(
            "[memory_noc] exists object: dec={:?}, {}",
            dec_id, object_id
        );

        Ok(self.cache.lock().unwrap().contains_key(object_id))
    }

    async fn get_object_map_ex(
        &self,
        dec_id: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapCacheItem>> {
        info!("[memory_noc] load object: dec={:?}, {}", dec_id, object_id);

        let cache = self.cache.lock().unwrap();
        Ok(cache.get(object_id).cloned())
    }

    async fn put_object_map(
        &self,
        dec_id: Option<ObjectId>,
        object_id: ObjectId,
        object: ObjectMap,
        access: Option<AccessString>,
    ) -> BuckyResult<()> {
        info!(
            "[memory_noc] save object: dec={:?}, {}, {:?}",
            dec_id, object_id, access
        );

        let item = ObjectMapCacheItem {
            object,
            access: access.unwrap_or(AccessString::default()),
        };

        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(object_id, item);
        }

        Ok(())
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////
///  同一个root共享的一个cache
#[async_trait::async_trait]
pub trait ObjectMapRootCache: Send + Sync {
    async fn exists(&self, object_id: &ObjectId) -> BuckyResult<bool>;

    async fn get_object_map(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectMapRef>> {
        self.get_object_map_ex(object_id)
            .await
            .map(|ret| ret.map(|v| v.object))
    }

    async fn get_object_map_ex(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapRefCacheItem>>;

    async fn put_object_map(
        &self,
        object_id: ObjectId,
        object: ObjectMap,
        access: Option<AccessString>,
    ) -> BuckyResult<()>;
}

pub type ObjectMapRootCacheRef = Arc<Box<dyn ObjectMapRootCache>>;

#[derive(Clone)]
pub struct ObjectMapRefCacheItem {
    pub object: ObjectMapRef,
    pub access: AccessString,
}

pub struct ObjectMapRootMemoryCache {
    // None for system dec
    dec_id: Option<ObjectId>,

    // 依赖的底层缓存，一般是noc层的缓存
    noc: ObjectMapNOCCacheRef,

    // 用来缓存从noc加载的objectmap和subs
    cache: Arc<Mutex<lru_time_cache::LruCache<ObjectId, ObjectMapRefCacheItem>>>,
}

impl ObjectMapRootMemoryCache {
    pub fn new(
        dec_id: Option<ObjectId>,
        noc: ObjectMapNOCCacheRef,
        timeout_in_secs: u64,
        capacity: usize,
    ) -> Self {
        let cache = lru_time_cache::LruCache::with_expiry_duration_and_capacity(
            std::time::Duration::from_secs(timeout_in_secs),
            capacity,
        );

        Self {
            dec_id,
            noc,
            cache: Arc::new(Mutex::new(cache)),
        }
    }

    pub fn new_ref(
        dec_id: Option<ObjectId>,
        noc: ObjectMapNOCCacheRef,
        timeout_in_secs: u64,
        capacity: usize,
    ) -> ObjectMapRootCacheRef {
        Arc::new(Box::new(Self::new(dec_id, noc, timeout_in_secs, capacity)))
    }

    pub fn new_default_ref(
        dec_id: Option<ObjectId>,
        noc: ObjectMapNOCCacheRef,
    ) -> ObjectMapRootCacheRef {
        Arc::new(Box::new(Self::new(dec_id, noc, 60 * 5, 1024)))
    }

    async fn exists(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        if self.cache.lock().unwrap().contains_key(object_id) {
            return Ok(true);
        }

        self.noc.exists(self.dec_id.clone(), object_id).await
    }

    async fn get_object_map_ex(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapRefCacheItem>> {
        let item = self.get_object_map_impl(object_id).await?;

        // FIXME 校验一次id是否是最新的
        if let Some(item) = &item {
            let current = item.object.lock().await;
            let real_id = current.flush_id_without_cache();
            assert_eq!(real_id, *object_id);

            let current_id = current.cached_object_id();
            assert_eq!(current_id, Some(real_id));
        }

        Ok(item)
    }

    async fn get_object_map_impl(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapRefCacheItem>> {
        // 首先查看缓存
        if let Some(v) = self.cache.lock().unwrap().get(object_id) {
            return Ok(Some(v.to_owned()));
        }

        // 最后尝试从noc加载
        let ret = self
            .noc
            .get_object_map_ex(self.dec_id.clone(), object_id)
            .await?;
        if ret.is_none() {
            return Ok(None);
        }

        let item = ret.unwrap();

        let object = Arc::new(AsyncMutex::new(item.object));

        let item = ObjectMapRefCacheItem {
            object,
            access: item.access,
        };

        // 缓存
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(_) = cache.insert(object_id.to_owned(), item.clone()) {
                warn!(
                    "insert objectmap to memory cache but already exists! id={}",
                    object_id
                );
            }
        }

        Ok(Some(item))
    }

    async fn put_object_map(
        &self,
        object_id: ObjectId,
        object: ObjectMap,
        access: Option<AccessString>,
    ) -> BuckyResult<()> {
        // TODO 这里是否需要更新缓存?

        // FIXME 校验一次id是否是最新的
        assert_eq!(Some(object_id), object.cached_object_id());
        let current_id = object.flush_id();
        assert_eq!(object_id, current_id);
        let real_id = object.flush_id_without_cache();
        assert_eq!(real_id, current_id);

        self.noc
            .put_object_map(self.dec_id.clone(), object_id, object, access)
            .await
    }
}

#[async_trait::async_trait]
impl ObjectMapRootCache for ObjectMapRootMemoryCache {
    async fn exists(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        Self::exists(&self, object_id).await
    }

    async fn get_object_map_ex(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapRefCacheItem>> {
        Self::get_object_map_ex(&self, object_id).await
    }

    async fn put_object_map(
        &self,
        object_id: ObjectId,
        object: ObjectMap,
        access: Option<AccessString>,
    ) -> BuckyResult<()> {
        Self::put_object_map(&self, object_id, object, access).await
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////
/// ObjectMap op_env操作粒度的cache
#[async_trait::async_trait]
pub trait ObjectMapOpEnvCache: Send + Sync {
    // from pending list and lower cache
    async fn get_object_map(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectMapRef>> {
        let ret = self.get_object_map_ex(object_id).await?;
        Ok(ret.map(|v| v.object))
    }

    async fn get_object_map_ex(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapRefCacheItem>>;

    // check if target object exists, in cache and in lower cache, not only for ObjectMap
    async fn exists(&self, object_id: &ObjectId) -> BuckyResult<bool>;

    // 同步的put，放置到暂存区
    fn put_object_map(
        &self,
        object_id: &ObjectId,
        object: ObjectMap,
        access: Option<AccessString>,
    ) -> BuckyResult<ObjectMapRef>;

    // 从暂存区移除先前已经提交的操作(put_sub之后，commit之前)
    fn remove_object_map(&self, object_id: &ObjectId) -> BuckyResult<ObjectMapRef>;

    // 提交所有的暂存操作到下一级缓存/存储
    async fn commit(&self) -> BuckyResult<()>;

    // gc before commit, clear all untouchable objects from target
    async fn gc(&self, single: bool, target: &ObjectId) -> BuckyResult<()>;

    // 清除所有待提交的对象
    fn abort(&self);
}

pub type ObjectMapOpEnvCacheRef = Arc<Box<dyn ObjectMapOpEnvCache>>;

struct ObjectMapPendingItem {
    is_touched: bool,
    item: ObjectMapRef,
    access: AccessString,
}

// 写操作缓存
struct ObjectMapOpEnvMemoryCachePendingList {
    pending: HashMap<ObjectId, ObjectMapPendingItem>,
}

impl ObjectMapOpEnvMemoryCachePendingList {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    fn exists(&mut self, object_id: &ObjectId) -> bool {
        self.pending.contains_key(object_id)
    }

    fn get_object_map(
        &mut self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapRefCacheItem>> {
        if let Some(v) = self.pending.get(object_id) {
            let item = ObjectMapRefCacheItem {
                object: v.item.clone(),
                access: v.access.clone(),
            };

            return Ok(Some(item));
        }

        // 缺页错误
        Ok(None)
    }

    fn remove_object_map(&mut self, object_id: &ObjectId) -> BuckyResult<ObjectMapRef> {
        match self.pending.remove(object_id) {
            Some(ret) => {
                info!("will remove pending objectmap from cache! id={}", object_id);
                Ok(ret.item)
            }
            None => {
                let msg = format!(
                    "remove pending objectmap from cache but not found! id={}",
                    object_id
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    fn put_object_map(
        &mut self,
        object_id: &ObjectId,
        object: ObjectMap,
        access: Option<AccessString>,
    ) -> BuckyResult<ObjectMapRef> {
        let object = Arc::new(AsyncMutex::new(object));

        match self.pending.entry(object_id.to_owned()) {
            Entry::Occupied(mut o) => {
                warn!(
                    "insert object to objectmap memory cache but already exists! id={}, access={:?}",
                    object_id, access,
                );
                let v = ObjectMapPendingItem {
                    is_touched: false,
                    item: object.clone(),
                    access: access.unwrap_or(AccessString::default()),
                };
                o.insert(v);
            }
            Entry::Vacant(v) => {
                debug!(
                    "insert object to objectmap memory cache! id={}, access={:?}",
                    object_id, access
                );
                let item = ObjectMapPendingItem {
                    is_touched: false,
                    item: object.clone(),
                    access: access.unwrap_or(AccessString::default()),
                };
                v.insert(item);
            }
        };

        Ok(object)
    }

    async fn commit(
        pending: HashMap<ObjectId, ObjectMapPendingItem>,
        root_cache: ObjectMapRootCacheRef,
    ) -> BuckyResult<()> {
        let count = pending.len();
        for (object_id, value) in pending {
            let obj = value.item;

            assert!(Arc::strong_count(&obj) == 1);
            let obj = Arc::try_unwrap(obj).unwrap();
            let obj = obj.into_inner();

            if let Err(e) = root_cache
                .put_object_map(object_id.clone(), obj, Some(value.access))
                .await
            {
                let msg = format!("commit pending objectmap error! obj={}, {}", object_id, e);
                error!("{}", msg);
                return Err(e);
            }
        }

        info!("commit all pending objectmap success! count={}", count);
        Ok(())
    }
}

pub struct ObjectMapOpEnvMemoryCache {
    // 依赖的底层root缓存
    root_cache: ObjectMapRootCacheRef,

    pending: Mutex<ObjectMapOpEnvMemoryCachePendingList>,
}

impl ObjectMapOpEnvMemoryCache {
    pub fn new(root_cache: ObjectMapRootCacheRef) -> Self {
        Self {
            root_cache,
            pending: Mutex::new(ObjectMapOpEnvMemoryCachePendingList::new()),
        }
    }

    pub fn new_ref(root_cache: ObjectMapRootCacheRef) -> ObjectMapOpEnvCacheRef {
        Arc::new(Box::new(Self::new(root_cache)))
    }
}

#[async_trait::async_trait]
impl ObjectMapOpEnvCache for ObjectMapOpEnvMemoryCache {
    async fn exists(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        if self.pending.lock().unwrap().exists(object_id) {
            return Ok(true);
        }

        self.root_cache.exists(object_id).await
    }

    async fn get_object_map_ex(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapRefCacheItem>> {
        // 首先从当前写缓存里面查找
        if let Some(obj) = self.pending.lock().unwrap().get_object_map(object_id)? {
            return Ok(Some(obj));
        }

        // 最后尝试从root_cache加载
        self.root_cache.get_object_map_ex(object_id).await
    }

    fn put_object_map(
        &self,
        object_id: &ObjectId,
        object: ObjectMap,
        access: Option<AccessString>,
    ) -> BuckyResult<ObjectMapRef> {
        self.pending
            .lock()
            .unwrap()
            .put_object_map(object_id, object, access)
    }

    fn remove_object_map(&self, object_id: &ObjectId) -> BuckyResult<ObjectMapRef> {
        self.pending.lock().unwrap().remove_object_map(object_id)
    }

    async fn commit(&self) -> BuckyResult<()> {
        // FIXME 这里提交操作会清空所有的pending对象，所以只能操作一次
        let mut pending = HashMap::new();
        {
            let mut inner = self.pending.lock().unwrap();
            std::mem::swap(&mut inner.pending, &mut pending);
        }
        ObjectMapOpEnvMemoryCachePendingList::commit(pending, self.root_cache.clone()).await
    }

    fn abort(&self) {
        self.pending.lock().unwrap().pending.clear();
    }

    async fn gc(&self, single: bool, target: &ObjectId) -> BuckyResult<()> {
        let mut pending = HashMap::new();
        {
            let mut inner = self.pending.lock().unwrap();
            std::mem::swap(&mut inner.pending, &mut pending);
        }

        let prev_count = pending.len();

        /*
        let mut total = 0;
        for (key, value) in pending.iter() {
            let len = key.raw_measure(&None).unwrap() + value.item.lock().await.raw_measure(&None).unwrap();
            total += len;
        }
        */

        let gc = ObjectMapOpEnvMemoryCacheGC::new(pending);
        let result = if single {
            gc.single_gc(target).await?
        } else {
            gc.path_gc(target).await?
        };

        let mut result: HashMap<ObjectId, ObjectMapPendingItem> = result
            .into_iter()
            .filter(|item| item.1.is_touched)
            .collect();

        info!(
            "gc for target single={}, target={}, {} -> {}",
            single,
            target,
            prev_count,
            result.len()
        );

        {
            let mut inner = self.pending.lock().unwrap();
            std::mem::swap(&mut inner.pending, &mut result);
        }

        Ok(())
    }
}

struct ObjectMapOpEnvMemoryCacheGC {
    pending: HashMap<ObjectId, ObjectMapPendingItem>,
}

impl ObjectMapOpEnvMemoryCacheGC {
    pub fn new(pending: HashMap<ObjectId, ObjectMapPendingItem>) -> Self {
        Self { pending }
    }

    fn touch_item(&mut self, id: &ObjectId) {
        debug!("gc touch item: {}", id);
        if id.is_data() {
            return;
        }

        if let Some(v) = self.pending.get_mut(id) {
            v.is_touched = true;
        }
    }

    pub async fn single_gc(
        mut self,
        target: &ObjectId,
    ) -> BuckyResult<HashMap<ObjectId, ObjectMapPendingItem>> {
        self.touch_item(target);

        let mut visitor = ObjectMapFullVisitor::new(Box::new(self));
        visitor.visit(target).await?;

        let loader = visitor.into_provider();
        let this = loader.into_any().downcast::<Self>().unwrap();
        Ok(this.pending)
    }

    pub async fn path_gc(
        mut self,
        root: &ObjectId,
    ) -> BuckyResult<HashMap<ObjectId, ObjectMapPendingItem>> {
        // first touch the root
        self.touch_item(root);

        let mut visitor = ObjectMapPathVisitor::new(Box::new(self));
        visitor.visit(root).await?;

        let visitor = visitor.into_provider();
        let this = visitor.into_any().downcast::<Self>().unwrap();
        Ok(this.pending)
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitLoader for ObjectMapOpEnvMemoryCacheGC {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    async fn get_object_map(&mut self, id: &ObjectId) -> BuckyResult<Option<ObjectMapRef>> {
        if let Some(v) = self.pending.get(id) {
            return Ok(Some(v.item.clone()));
        }

        debug!("object not exists: {}", id);
        // 缺页错误
        Ok(None)
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitor for ObjectMapOpEnvMemoryCacheGC {
    async fn visit_hub_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        self.touch_item(&item);

        Ok(())
    }

    async fn visit_map_item(&mut self, _key: &str, item: &ObjectId) -> BuckyResult<()> {
        self.touch_item(&item);

        Ok(())
    }

    async fn visit_set_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        self.touch_item(&item);

        Ok(())
    }

    async fn visit_diff_map_item(
        &mut self,
        _key: &str,
        item: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        if let Some(id) = &item.diff {
            self.touch_item(&id);
        }

        Ok(())
    }

    async fn visit_diff_set_item(&mut self, _item: &ObjectMapDiffSetItem) -> BuckyResult<()> {
        Ok(())
    }
}

impl ObjectMapVisitorProvider for ObjectMapOpEnvMemoryCacheGC {}
