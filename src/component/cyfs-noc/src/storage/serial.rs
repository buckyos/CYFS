use cyfs_base::*;
use cyfs_lib::*;

use async_std::sync::Mutex as AsyncMutex;
use cyfs_debug::Mutex;
use std::collections::{hash_map::Entry, HashMap};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

struct SerializeExecutorLock {
    lock: AsyncMutex<u32>,
    count: AtomicI32,
}

impl SerializeExecutorLock {
    pub fn new() -> Self {
        Self {
            lock: AsyncMutex::new(0),
            count: AtomicI32::new(1),
        }
    }

    pub fn add_ref(&self) -> i32 {
        let count = self.count.fetch_add(1, Ordering::SeqCst);
        assert!(count >= 0);

        count + 1
    }

    pub fn release(&self) -> i32 {
        let count = self.count.fetch_sub(1, Ordering::SeqCst);
        assert!(count > 0);

        count - 1
    }

    pub fn ref_count(&self) -> i32 {
        let count = self.count.load(Ordering::SeqCst);
        assert!(count >= 0);

        count
    }
}

type SerializeExecutorLockRef = Arc<SerializeExecutorLock>;

pub struct NamedObjectCacheSerializer {
    next: NamedObjectCacheRef,
    locks: Mutex<HashMap<ObjectId, SerializeExecutorLockRef>>,
}

impl NamedObjectCacheSerializer {
    pub fn new(next: NamedObjectCacheRef) -> Self {
        Self {
            next,
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn acquire_lock(&self, object_id: &ObjectId) -> SerializeExecutorLockRef {
        let lock = {
            let mut locks = self.locks.lock().unwrap();

            match locks.entry(object_id.to_owned()) {
                Entry::Vacant(v) => {
                    let item = SerializeExecutorLock::new();
                    v.insert(Arc::new(item)).clone()
                }
                Entry::Occupied(o) => {
                    let item = o.get().clone();
                    item.add_ref();
                    item.clone()
                }
            }
        };

        lock
    }

    fn leave_lock(&self, object_id: &ObjectId, lock: SerializeExecutorLockRef) {
        let ref_count = lock.release();
        if ref_count <= 0 {
            self.try_release_lock(object_id, lock);
        }
    }

    fn try_release_lock(&self, object_id: &ObjectId, lock: SerializeExecutorLockRef) {
        let mut locks = self.locks.lock().unwrap();

        // should check once before been removed!!
        if lock.ref_count() <= 0 {
            locks.remove_entry(object_id).unwrap();
        }
    }
}

#[async_trait::async_trait]
impl NamedObjectCache for NamedObjectCacheSerializer {
    async fn put_object(
        &self,
        req: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse> {
        let lock = self.acquire_lock(&req.object.object_id);
        let ret = {
            let _guard = lock.lock.lock().await;
            self.next.put_object(req).await
        };

        self.leave_lock(&req.object.object_id, lock);

        ret
    }

    async fn get_object_raw(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
        let lock = self.acquire_lock(&req.object_id);
        let ret = {
            let _guard = lock.lock.lock().await;
            self.next.get_object_raw(req).await
        };

        self.leave_lock(&req.object_id, lock);

        ret
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse> {
        let lock = self.acquire_lock(&req.object_id);
        let ret = {
            let _guard = lock.lock.lock().await;
            self.next.delete_object(req).await
        };

        self.leave_lock(&req.object_id, lock);

        ret
    }

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse> {
        let lock = self.acquire_lock(&req.object_id);
        let ret = {
            let _guard = lock.lock.lock().await;
            self.next.exists_object(req).await
        };

        self.leave_lock(&req.object_id, lock);

        ret
    }

    async fn update_object_meta(
        &self,
        req: &NamedObjectCacheUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        let lock = self.acquire_lock(&req.object_id);
        let ret = {
            let _guard = lock.lock.lock().await;
            self.next.update_object_meta(req).await
        };

        self.leave_lock(&req.object_id, lock);

        ret
    }

    async fn check_object_access(
        &self,
        req: &NamedObjectCacheCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        let lock = self.acquire_lock(&req.object_id);
        let ret = {
            let _guard = lock.lock.lock().await;
            self.next.check_object_access(req).await
        };

        self.leave_lock(&req.object_id, lock);

        ret
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        self.next.stat().await
    }

    fn bind_object_meta_access_provider(
        &self,
        object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    ) {
        self.next
            .bind_object_meta_access_provider(object_meta_access_provider)
    }
}
