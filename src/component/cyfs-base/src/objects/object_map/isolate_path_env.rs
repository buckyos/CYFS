use super::access::OpEnvPathAccess;
use super::cache::*;
use super::iterator::*;
use super::path::*;
use super::root::ObjectMapRootHolder;
use crate::*;

use async_std::sync::Mutex as AsyncMutex;
use once_cell::sync::OnceCell;
use std::sync::Arc;

// 每个root共享一个大的读cache，每个op_env都有独立的写cache，在commit时候提交
pub struct ObjectMapIsolatePathOpEnv {
    // 每个root下的op_env都有唯一的一个sid
    sid: u64,

    // 用以创建objectmap
    owner: Option<ObjectId>,
    dec_id: Option<ObjectId>,

    // 当前op_env的所属root
    root_holder: ObjectMapRootHolder,

    path: OnceCell<ObjectMapPath>,

    // env级别的cache
    cache: ObjectMapOpEnvCacheRef,

    // 写锁，确保顺序写
    write_lock: AsyncMutex<()>,

    // 权限相关
    access: Option<OpEnvPathAccess>,
}

impl ObjectMapIsolatePathOpEnv {
    pub(crate) fn new(
        sid: u64,
        root_holder: &ObjectMapRootHolder,
        root_cache: &ObjectMapRootCacheRef,
        owner: Option<ObjectId>,
        dec_id: Option<ObjectId>,
        access: Option<OpEnvPathAccess>,
    ) -> Self {
        debug!("new isolate_path_op_env: sid={},", sid);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        Self {
            sid,
            root_holder: root_holder.clone(),
            path: OnceCell::new(),
            cache,
            owner,
            dec_id,
            write_lock: AsyncMutex::new(()),
            access,
        }
    }

    fn init_path(&self, root: ObjectId) -> BuckyResult<()> {
        let path = ObjectMapPath::new(root.clone(), self.cache.clone(), false);
        if let Err(_) = self.path.set(path) {
            let msg = format!(
                "isolate_path_op_env has been initialized already! current root={}",
                self.path.get().unwrap().root()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
        }

        Ok(())
    }

    fn path(&self) -> BuckyResult<&ObjectMapPath> {
        self.path.get().ok_or(BuckyError::new(
            BuckyErrorCode::ErrorState,
            "isolate_path_op_env has not been initialized yet!",
        ))
    }

    // init methods
    pub async fn create_new(&self, content_type: ObjectMapSimpleContentType) -> BuckyResult<()> {
        let obj = ObjectMap::new(
            content_type.clone(),
            self.owner.clone(),
            self.dec_id.clone(),
        )
        .no_create_time()
        .build();
        let id = obj.flush_id();
        info!(
            "create new objectmap for ioslate_path_op_env: content_type={:?}, id={}",
            content_type, id
        );

        self.cache.put_object_map(&id, obj, None)?;
        self.init_path(id)
    }

    pub async fn load(&self, obj_map_id: &ObjectId) -> BuckyResult<()> {
        let ret = self.cache.get_object_map(obj_map_id).await?;
        if ret.is_none() {
            let msg = format!(
                "load ioslate_path_op_env object_id but not found! id={}",
                obj_map_id,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        debug!("load objectmap for ioslate_path_op_env: id={}", obj_map_id,);
        self.init_path(obj_map_id.to_owned())
    }

    pub async fn load_by_path(&self, full_path: &str) -> BuckyResult<()> {
        let (path, key) = ObjectMapPath::parse_path_allow_empty_key(full_path)?;

        self.load_by_key(path, key).await
    }

    pub async fn load_with_inner_path(
        &self,
        obj_map_id: &ObjectId,
        inner_path: Option<String>,
    ) -> BuckyResult<()> {
        let value = match &inner_path {
            Some(inner_path) if inner_path.len() > 0 => {
                let object_path = ObjectMapPath::new(obj_map_id.clone(), self.cache.clone(), false);
                let value = object_path.get_by_path(&inner_path).await?;
                if value.is_none() {
                    let msg = format!(
                        "load ioslate_path_op_env with inner_path but not found! root={}, inner_path={}",
                        obj_map_id, inner_path,
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }

                value.unwrap()
            }
            _ => obj_map_id.to_owned(),
        };

        info!(
            "will load ioslate_path_op_env with inner_path! root={}, inner_path={:?}, target={}",
            obj_map_id, inner_path, value,
        );

        self.load(&value).await
    }

    // 加载指定路径上的object_map
    // root不能使用single_op_env直接操作，所以必须至少要指定一个key
    pub async fn load_by_key(&self, path: &str, key: &str) -> BuckyResult<()> {
        // First check access permissions!
        if let Some(access) = &self.access {
            access.check_path_key(path, key, RequestOpType::Read)?;
        }

        let root = self.root_holder.get_current_root();

        let value = if key.len() > 0 {
            let object_path = ObjectMapPath::new(root.clone(), self.cache.clone(), false);
            let value = object_path.get_by_key(path, key).await?;
            if value.is_none() {
                let msg = format!(
                    "load ioslate_path_op_env by path but not found! root={}, path={}, key={}",
                    root, path, key
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }

            value.unwrap()
        } else {
            assert_eq!(path, "/");
            root
        };

        info!(
            "will load ioslate_path_op_env by path! root={}, path={}, key={}, value={}",
            root, path, key, value
        );

        self.load(&value).await
    }

    pub fn cache(&self) -> &ObjectMapOpEnvCacheRef {
        &self.cache
    }

    pub fn sid(&self) -> u64 {
        self.sid
    }

    // 调用次方法会导致path快照被绑定，所以如果需要lock，那么需要按照create_op_env->lock->访问其它方法的次序操作
    pub fn root(&self) -> Option<ObjectId> {
        self.path.get().map(|path| path.root())
    }

    // list
    pub async fn list(&self, path: &str) -> BuckyResult<ObjectMapContentList> {
        self.path()?.list(path).await
    }

    // metadata
    pub async fn metadata(&self, path: &str) -> BuckyResult<ObjectMapMetaData> {
        self.path()?.metadata(path).await
    }

    // map path methods
    pub async fn get_by_path(&self, full_path: &str) -> BuckyResult<Option<ObjectId>> {
        self.path()?.get_by_path(full_path).await
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

        let _write_lock = self.write_lock.lock().await;
        self.path()?
            .create_new_with_path(full_path, content_type)
            .await
    }

    pub async fn insert_with_path(&self, full_path: &str, value: &ObjectId) -> BuckyResult<()> {
        info!(
            "op_path_env insert_with_path: sid={}, full_path={}, value={}",
            self.sid, full_path, value
        );

        let _write_lock = self.write_lock.lock().await;
        self.path()?.insert_with_path(full_path, value).await
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

        let _write_lock = self.write_lock.lock().await;
        self.path()?
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

        let _write_lock = self.write_lock.lock().await;
        self.path()?.remove_with_path(full_path, prev_value).await
    }

    // map origin methods
    pub async fn get_by_key(&self, path: &str, key: &str) -> BuckyResult<Option<ObjectId>> {
        self.path()?.get_by_key(path, key).await
    }

    pub async fn create_new_with_key(
        &self,
        path: &str,
        key: &str,
        content_type: ObjectMapSimpleContentType,
    ) -> BuckyResult<()> {
        info!(
            "op_path_env create_new: sid={}, path={}, key={}, content_type={:?}",
            self.sid, path, key, content_type,
        );

        let _write_lock = self.write_lock.lock().await;
        self.path()?.create_new(path, key, content_type).await
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

        let _write_lock = self.write_lock.lock().await;
        self.path()?.insert_with_key(path, key, value).await
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

        let _write_lock = self.write_lock.lock().await;
        self.path()?
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

        let _write_lock = self.write_lock.lock().await;
        self.path()?.remove_with_key(path, key, prev_value).await
    }

    // set methods
    pub async fn contains(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        self.path()?.contains(path, object_id).await
    }

    pub async fn insert(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        info!(
            "op_path_env insert: sid={}, path={}, value={}",
            self.sid, path, object_id,
        );

        let _write_lock = self.write_lock.lock().await;
        self.path()?.insert(path, object_id).await
    }

    pub async fn remove(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        info!(
            "op_path_env remove: sid={}, path={}, value={}",
            self.sid, path, object_id,
        );

        let _write_lock = self.write_lock.lock().await;
        self.path()?.remove(path, object_id).await
    }

    pub async fn update(&self) -> BuckyResult<ObjectId> {
        let _write_lock = self.write_lock.lock().await;

        // 提交所有pending的对象到noc
        let root = self.path()?.root();
        if let Err(e) = self.cache.gc(false, &root).await {
            error!("path env's cache gc error! root={}, {}", root, e);
        }

        self.cache.commit().await?;

        Ok(root)
    }

    // 提交操作，只可以调用一次
    // 提交成功，返回最新的root id
    pub async fn commit(self) -> BuckyResult<ObjectId> {
        self.update().await
    }

    pub fn abort(self) -> BuckyResult<()> {
        info!("will abort isolate_path_op_env: sid={}", self.sid);

        // 释放cache里面的pending
        self.cache.abort();

        Ok(())
    }
}

#[derive(Clone)]
pub struct ObjectMapIsolatePathOpEnvRef(Arc<ObjectMapIsolatePathOpEnv>);

impl ObjectMapIsolatePathOpEnvRef {
    pub fn new(env: ObjectMapIsolatePathOpEnv) -> Self {
        Self(Arc::new(env))
    }

    fn into_raw(self) -> BuckyResult<ObjectMapIsolatePathOpEnv> {
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

impl std::ops::Deref for ObjectMapIsolatePathOpEnvRef {
    type Target = Arc<ObjectMapIsolatePathOpEnv>;
    fn deref(&self) -> &Arc<ObjectMapIsolatePathOpEnv> {
        &self.0
    }
}
