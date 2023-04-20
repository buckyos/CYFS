use super::access::OpEnvPathAccess;
use super::cache::*;
use super::iterator::*;
use super::lock::*;
use super::path::*;
use super::root::ObjectMapRootHolder;
use crate::*;

use async_std::sync::Mutex as AsyncMutex;
use once_cell::sync::OnceCell;
use std::sync::{Arc, RwLock};

//#[derive(Clone)]

pub struct ObjectMapPathSnapshot {
    // 记录的初始状态的root
    root: RwLock<ObjectId>,

    // 所有基于path的操作都在这里面实现，包括root的更新
    path: ObjectMapPath,
}

// 每个root共享一个大的读cache，每个op_env都有独立的写cache，在commit时候提交
pub struct ObjectMapPathOpEnv {
    // 每个root下的op_env都有唯一的一个sid
    sid: u64,

    // 当前op_env的所属root
    root_holder: ObjectMapRootHolder,

    path: OnceCell<ObjectMapPathSnapshot>,

    // 同一个root下共享一个全局的锁管理器
    lock: ObjectMapPathLock,

    // env级别的cache
    cache: ObjectMapOpEnvCacheRef,

    // 写锁，确保顺序写
    write_lock: AsyncMutex<()>,

    // 权限相关
    access: Option<OpEnvPathAccess>,
}

impl Drop for ObjectMapPathOpEnv {
    fn drop(&mut self) {
        async_std::task::block_on(self.unlock());
    }
}

impl ObjectMapPathOpEnv {
    pub(crate) fn new(
        sid: u64,
        root_holder: &ObjectMapRootHolder,
        lock: &ObjectMapPathLock,
        root_cache: &ObjectMapRootCacheRef,
        access: Option<OpEnvPathAccess>,
    ) -> Self {
        debug!("new path_op_env: sid={},", sid);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        Self {
            sid,
            root_holder: root_holder.clone(),
            path: OnceCell::new(),
            cache,
            lock: lock.clone(),
            write_lock: AsyncMutex::new(()),
            access,
        }
    }

    // 获取快照，如果不存在则会创建
    fn path_snapshot(&self) -> &ObjectMapPathSnapshot {
        self.path.get_or_init(|| {
            // 记录当前root的快照
            let root = self.root_holder.get_current_root();
            info!(
                "path_op_env bind root snapshot: sid={}, root={}",
                self.sid, root
            );
            let path = ObjectMapPath::new(root.clone(), self.cache.clone(), true);

            ObjectMapPathSnapshot {
                root: RwLock::new(root),
                path,
            }
        })
    }

    pub fn cache(&self) -> &ObjectMapOpEnvCacheRef {
        &self.cache
    }

    pub fn sid(&self) -> u64 {
        self.sid
    }

    // 调用次方法会导致path快照被绑定，所以如果需要lock，那么需要按照create_op_env->lock->访问其它方法的次序操作
    pub fn root(&self) -> ObjectId {
        self.path_snapshot().root.read().unwrap().to_owned()
    }

    fn path(&self) -> &ObjectMapPath {
        &self.path_snapshot().path
    }

    pub async fn lock_path(
        &self,
        path_list: Vec<String>,
        duration_in_millsecs: u64,
        as_try: bool,
    ) -> BuckyResult<()> {
        info!(
            "path_op_env lock_path: sid={}, path_list={:?}, duration_in_millsecs={}",
            self.sid, path_list, duration_in_millsecs
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path_list(&path_list, RequestOpType::Write)?;
        }

        let mut req_list = vec![];
        let expired = if duration_in_millsecs > 0 {
            let now = bucky_time_now();
            if duration_in_millsecs < (u64::MAX - now) / 1000 {
                now + duration_in_millsecs * 1000
            } else {
                duration_in_millsecs
            }
        } else {
            0
        };

        for path in path_list {
            let req = PathLockRequest {
                path,
                sid: self.sid,
                expired,
            };

            req_list.push(req);
        }

        if as_try {
            self.lock.try_lock_list(req_list).await
        } else {
            self.lock.lock_list(req_list).await;
            Ok(())
        }
    }

    // list
    pub async fn list(&self, path: &str) -> BuckyResult<ObjectMapContentList> {
        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(path, RequestOpType::Read)?;
        }
        self.path().list(path).await
    }

    // metadata
    pub async fn metadata(&self, path: &str) -> BuckyResult<ObjectMapMetaData> {
        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(path, RequestOpType::Read)?;
        }
        self.path().metadata(path).await
    }

    // map path methods
    pub async fn get_by_path(&self, full_path: &str) -> BuckyResult<Option<ObjectId>> {
        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(full_path, RequestOpType::Read)?;
        }

        self.path().get_by_path(full_path).await
    }

    pub async fn create_new_with_path(
        &self,
        full_path: &str,
        content_type: ObjectMapSimpleContentType,
    ) -> BuckyResult<()> {
        info!(
            "op_path_env create_new_with_path: sid={}, path={}, content_type={:?}",
            self.sid, full_path, content_type,
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(full_path, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock.try_enter_path(full_path, self.sid).await?;
        self.path()
            .create_new_with_path(full_path, content_type)
            .await
    }

    pub async fn insert_with_path(&self, full_path: &str, value: &ObjectId) -> BuckyResult<()> {
        info!(
            "op_path_env insert_with_path: sid={}, full_path={}, value={}",
            self.sid, full_path, value
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(full_path, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock.try_enter_path(full_path, self.sid).await?;
        self.path().insert_with_path(full_path, value).await
    }

    pub async fn set_with_path(
        &self,
        full_path: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        info!(
            "op_path_env set_with_path: sid={}, full_path={}, value={}, prev_value={:?}, auto_insert={}",
             self.sid, full_path, value, prev_value, auto_insert,
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(full_path, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock.try_enter_path(full_path, self.sid).await?;
        self.path()
            .set_with_path(full_path, value, prev_value, auto_insert)
            .await
    }

    pub async fn remove_with_path(
        &self,
        full_path: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        info!(
            "op_path_env remove_with_path: sid={}, full_path={}, prev_value={:?}",
            self.sid, full_path, prev_value,
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(full_path, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock.try_enter_path(full_path, self.sid).await?;
        self.path().remove_with_path(full_path, prev_value).await
    }

    // map origin methods
    pub async fn get_by_key(&self, path: &str, key: &str) -> BuckyResult<Option<ObjectId>> {
        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_path_key(path, key, RequestOpType::Read)?;
        }

        self.path().get_by_key(path, key).await
    }

    pub async fn create_new(
        &self,
        path: &str,
        key: &str,
        content_type: ObjectMapSimpleContentType,
    ) -> BuckyResult<()> {
        info!(
            "op_path_env create_new: sid={}, path={}, key={}, content_type={:?}",
            self.sid, path, key, content_type,
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_path_key(path, key, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock
            .try_enter_path_and_key(path, key, self.sid)
            .await?;

        self.path().create_new(path, key, content_type).await
    }

    pub async fn insert_with_key(
        &self,
        path: &str,
        key: &str,
        value: &ObjectId,
    ) -> BuckyResult<()> {
        info!(
            "op_path_env insert_with_key: sid={}, path={}, key={}, value={}",
            self.sid, path, key, value
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_path_key(path, key, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock
            .try_enter_path_and_key(path, key, self.sid)
            .await?;
        self.path().insert_with_key(path, key, value).await
    }

    pub async fn set_with_key(
        &self,
        path: &str,
        key: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        info!(
            "op_path_env set_with_key: sid={}, path={}, key={}, value={}, prev_value={:?}, auto_insert={}",
             self.sid, path, key, value, prev_value, auto_insert,
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_path_key(path, key, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock
            .try_enter_path_and_key(path, key, self.sid)
            .await?;
        self.path()
            .set_with_key(path, key, value, prev_value, auto_insert)
            .await
    }

    pub async fn remove_with_key(
        &self,
        path: &str,
        key: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        info!(
            "op_path_env remove_with_key: sid={}, path={}, key={}, prev_value={:?}",
            self.sid, path, key, prev_value,
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_path_key(path, key, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock
            .try_enter_path_and_key(path, key, self.sid)
            .await?;
        self.path().remove_with_key(path, key, prev_value).await
    }

    // set methods
    pub async fn contains(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(path, RequestOpType::Read)?;
        }

        self.path().contains(path, object_id).await
    }

    pub async fn insert(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        info!(
            "op_path_env insert: sid={}, path={}, value={}",
            self.sid, path, object_id,
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(path, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock.try_enter_path(path, self.sid).await?;
        self.path().insert(path, object_id).await
    }

    pub async fn remove(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        info!(
            "op_path_env remove: sid={}, path={}, value={}",
            self.sid, path, object_id,
        );

        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_full_path(path, RequestOpType::Write)?;
        }

        let _write_lock = self.write_lock.lock().await;
        self.lock.try_enter_path(path, self.sid).await?;
        self.path().remove(path, object_id).await
    }

    async fn update_root(&self) -> BuckyResult<ObjectId> {
        // 首先判断有没有发生写操作，会导致path.root改变
        let new_root = self.path().root();
        let current_root = self.root();
        if new_root == current_root {
            info!(
                "op env commit but root not changed! sid={}, root={}",
                self.sid, current_root
            );
            return Ok(new_root);
        }

        let this = &self;
        let update = |root: ObjectId| async move {
            // 事务提交时候，存在两种情况:
            // 1. root在外部没有发生改变，那么直接把暂存的操作提交到noc，并切换root到当前path的最新root
            // 2. root在外部发生改变了，那么需要更新path的root到最新状态，并以事务模式尝试提交，提交成功后，切换root到当前path的最新root
            if root != current_root {
                info!("path_op_env commit but root changed, now will redo op list! sid={}, current_root={}, new_root={}", 
                    this.sid, current_root, root);

                this.cache.abort();

                // root在外部被修改了，那么需要重做op_list
                this.path().update_root(root.clone(), &new_root)?;

                info!(
                    "will commit op list on root changed: {} -> {}",
                    current_root, root
                );
                this.path().commit_op_list().await?;
            } else {
                // env操作期间，root没发生改变，那么不再重新执行op_list
                info!("will clear op list because root not changed during the operations: {}", root);
                this.path().clear_op_list();
            }

            // update current op_env's snapshot
            let new_root = this.path().root();
            *this.path_snapshot().root.write().unwrap() = new_root.clone();

            // 提交所有pending的对象到noc
            if let Err(e) = this.cache.gc(false, &new_root).await {
                error!("path env's cache gc error! root={}, {}", root, e);
            }

            this.cache.commit().await?;

            Ok(new_root)
        };

        let new_root = self.root_holder.update_root(Box::new(update)).await?;

        Ok(new_root)
    }

    pub async fn update(&self) -> BuckyResult<ObjectId> {
        let _write_lock = self.write_lock.lock().await;
        self.update_root().await
    }

    // 提交操作，只可以调用一次
    // 提交成功，返回最新的root id
    pub async fn commit(self) -> BuckyResult<ObjectId> {
        self.update_root().await
    }

    // 释当前session持有的所有lock
    async fn unlock(&self) {
        let req = PathUnlockRequest {
            path: None,
            sid: self.sid,
        };

        self.lock.unlock(req).await.unwrap();
    }

    pub fn abort(self) -> BuckyResult<()> {
        info!("will abort path_op_env: sid={}", self.sid);

        // 释放cache里面的pending
        self.cache.abort();

        Ok(())
    }
}

#[derive(Clone)]
pub struct ObjectMapPathOpEnvRef(Arc<ObjectMapPathOpEnv>);

impl ObjectMapPathOpEnvRef {
    pub fn new(env: ObjectMapPathOpEnv) -> Self {
        Self(Arc::new(env))
    }

    fn into_raw(self) -> BuckyResult<ObjectMapPathOpEnv> {
        let sid = self.sid();
        let env = Arc::try_unwrap(self.0).map_err(|this| {
            let msg = format!(
                "path_op_env's ref_count is more than one! sid={}, ref={}",
                sid,
                Arc::strong_count(&this)
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::ErrorState, msg)
        })?;

        Ok(env)
    }

    pub fn is_dropable(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }

    pub async fn commit(self) -> BuckyResult<ObjectId> {
        let env = self.into_raw()?;

        env.commit().await
    }

    pub fn abort(self) -> BuckyResult<()> {
        let env = self.into_raw()?;

        env.abort()
    }
}

impl std::ops::Deref for ObjectMapPathOpEnvRef {
    type Target = Arc<ObjectMapPathOpEnv>;
    fn deref(&self) -> &Arc<ObjectMapPathOpEnv> {
        &self.0
    }
}
