use super::access::OpEnvPathAccess;
use super::cache::*;
use super::iterator::*;
use super::object_map::*;
use super::path::*;
use crate::*;

use async_std::sync::Mutex as AsyncMutex;
use once_cell::sync::OnceCell;
use std::sync::Arc;

pub struct ObjectMapSingleOpEnv {
    sid: u64,

    // 所属dec的root
    root_holder: ObjectMapRootHolder,

    // 操作的目标object_map
    root: AsyncMutex<Option<ObjectMap>>,

    // env级别的cache
    cache: ObjectMapOpEnvCacheRef,

    iterator: OnceCell<AsyncMutex<ObjectMapIterator>>,

    // 权限相关
    access: Option<OpEnvPathAccess>,
}

impl ObjectMapSingleOpEnv {
    pub(crate) fn new(
        sid: u64,
        root_holder: &ObjectMapRootHolder,
        root_cache: &ObjectMapRootCacheRef,
        access: Option<OpEnvPathAccess>,
    ) -> Self {
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        Self {
            sid,
            root_holder: root_holder.clone(),
            root: AsyncMutex::new(None),
            cache,
            iterator: OnceCell::new(),
            access,
        }
    }

    pub fn sid(&self) -> u64 {
        self.sid
    }

    // 获取当前操作的object_map id，需要注意在commit之前都是快照模式，id不会更新
    pub async fn get_current_root(&self) -> Option<ObjectId> {
        let ret = self.root.lock().await;
        ret.as_ref().map(|v| v.cached_object_id().unwrap())
    }

    async fn set_root(&self, obj_map: ObjectMap) -> BuckyResult<()> {
        let mut current = self.root.lock().await;
        if current.is_some() {
            let msg = format!(
                "single op_env root already been set! id={}",
                current.as_ref().unwrap().cached_object_id().unwrap()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
        }

        info!(
            "single op_env root init: id={}",
            obj_map.cached_object_id().unwrap()
        );

        *current = Some(obj_map);

        Ok(())
    }

    // 创建一个新的object_map
    pub async fn create_new(&self, content_type: ObjectMapSimpleContentType, owner: Option<ObjectId>, dec_id: Option<ObjectId>,) -> BuckyResult<()> {
        let obj = ObjectMap::new(
            content_type.clone(),
            owner,
            dec_id,
        )
        .no_create_time()
        .build();
        let id = obj.flush_id();
        info!(
            "create new objectmap for single op_env: content_type={:?}, id={}",
            content_type, id
        );

        self.set_root(obj).await?;

        Ok(())
    }

    // 加载一个已有的object_map
    pub async fn load(&self, obj_map_id: &ObjectId) -> BuckyResult<()> {
        let ret = self.cache.get_object_map(obj_map_id).await?;
        if ret.is_none() {
            let msg = format!(
                "load single op_env object_id but not found! id={}",
                obj_map_id,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        debug!("load objectmap for single op_env: id={}", obj_map_id,);

        // 拷贝一份用以后续的修改操作
        let obj_map = ret.unwrap().lock().await.clone();
        self.set_root(obj_map).await?;

        Ok(())
    }

    pub async fn load_by_path(&self, full_path: &str) -> BuckyResult<()> {
        let (path, key) = ObjectMapPath::parse_path_allow_empty_key(full_path)?;

        self.load_by_key(path, key).await
    }

    pub async fn load_with_inner_path(&self, obj_map_id: &ObjectId, inner_path: Option<String>) -> BuckyResult<()> {
        let value = match &inner_path {
            Some(inner_path) if inner_path.len() > 0  => {
                let object_path = ObjectMapPath::new(obj_map_id.clone(), self.cache.clone(), false);
                let value = object_path.get_by_path(&inner_path).await?;
                if value.is_none() {
                    let msg = format!(
                        "load single_op_env with inner_path but not found! root={}, inner_path={}",
                        obj_map_id, inner_path,
                    );
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }
    
                value.unwrap()
            }
            _ => {
                obj_map_id.to_owned()
            }
        };

        info!(
            "will load single_op_env with inner_path! root={}, inner_path={:?}, target={}",
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
                    "load single_op_env by path but not found! root={}, path={}, key={}",
                    root, path, key
                );
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }

            value.unwrap()
        } else {
            assert_eq!(path, "/");
            root
        };

        info!(
            "will load single_op_env by path! root={}, path={}, key={}, value={}",
            root, path, key, value
        );

        self.load(&value).await
    }

    // list
    pub async fn list(&self) -> BuckyResult<ObjectMapContentList> {
        let ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!("single op_env root not been init yet! sid={}", self.sid);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        let obj = ret.as_ref().unwrap();
        let mut list = ObjectMapContentList::new(obj.count() as usize);
        ret.as_ref().unwrap().list(&self.cache, &mut list).await?;

        Ok(list)
    }

    // iterator
    pub async fn next(&self, step: usize) -> BuckyResult<ObjectMapContentList> {
        let ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!("single op_env root not been init yet! sid={}", self.sid);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        let obj = ret.as_ref().unwrap();

        let iterator = self.iterator.get_or_init(|| {
            let ret = ObjectMapIterator::new(false, &obj, self.cache.clone());
            AsyncMutex::new(ret)
        });

        let mut it = iterator.lock().await;
        it.next(&obj, step).await
    }

    // reset the iterator
    pub async fn reset(&self) {
        if self.iterator.get().is_none() {
            return;
        }

        let ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!("single op_env root not been init yet! sid={}", self.sid);
            error!("{}", msg);
            return;
        }

        let obj = ret.as_ref().unwrap();

        let ret = self.iterator.get();
        if ret.is_none() {
            return;
        }

        let new_it = ObjectMapIterator::new(false, &obj, self.cache.clone());

        info!(
            "will reset single op_env iterator: root={}",
            obj.cached_object_id().unwrap()
        );

        let iterator = ret.unwrap();
        *iterator.lock().await = new_it;
    }

    pub async fn metadata(&self) -> BuckyResult<ObjectMapMetaData> {
        let ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!("single op_env root not been init yet! sid={}", self.sid);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        let obj = ret.as_ref().unwrap();
        Ok(obj.metadata())
    }

    // map methods
    pub async fn get_by_key(&self, key: &str) -> BuckyResult<Option<ObjectId>> {
        let ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!("single op_env root not been init yet! sid={}", self.sid);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        ret.as_ref().unwrap().get_by_key(&self.cache, key).await
    }

    pub async fn insert_with_key(&self, key: &str, value: &ObjectId) -> BuckyResult<()> {
        let mut ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!(
                "single op_env root not been init yet! key={}, value={}",
                key, value
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        ret.as_mut()
            .unwrap()
            .insert_with_key(&self.cache, key, value)
            .await
    }

    pub async fn set_with_key(
        &self,
        key: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!(
                "single op_env root not been init yet! sid={}, key={}, value={}",
                self.sid, key, value
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        ret.as_mut()
            .unwrap()
            .set_with_key(&self.cache, key, value, prev_value, auto_insert)
            .await
    }

    pub async fn remove_with_key(
        &self,
        key: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!(
                "single op_env root not been init yet! sid={}, key={}",
                self.sid, key
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        ret.as_mut()
            .unwrap()
            .remove_with_key(&self.cache, key, prev_value)
            .await
    }

    // set methods
    pub async fn contains(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!(
                "single op_env root not been init yet! sid={}, value={}",
                self.sid, object_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        ret.as_ref().unwrap().contains(&self.cache, object_id).await
    }

    pub async fn insert(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let mut ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!(
                "single op_env root not been init yet! sid={}, value={}",
                self.sid, object_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        ret.as_mut().unwrap().insert(&self.cache, object_id).await
    }

    pub async fn remove(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let mut ret = self.root.lock().await;
        if ret.is_none() {
            let msg = format!(
                "single op_env root not been init yet! sid={}, value={}",
                self.sid, object_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        ret.as_mut().unwrap().remove(&self.cache, object_id).await
    }

    async fn update_root(&self, finish: bool) -> BuckyResult<ObjectId> {
        let mut root_slot = self.root.lock().await;
        if root_slot.is_none() {
            let msg = format!(
                "update root error, single op_env root not been init yet! sid={}",
                self.sid
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        let root = root_slot.as_ref().unwrap();
        let object_id = root.cached_object_id().unwrap();
        let new_id = root.flush_id();
        if object_id == new_id {
            info!(
                "single op_env update root but object_id unchanged! id={}",
                object_id
            );
            return Ok(new_id);
        }

        // 发生改变了，需要提交到noc
        info!(
            "single op_env root object changed! sid={}, {} => {}",
            self.sid, object_id, new_id
        );

        let root = if finish {
            root_slot.take().unwrap()
        } else {
            root.clone()
        };

        self.cache.put_object_map(&new_id, root, None)?;

        if let Err(e) = self.cache.gc(true, &new_id).await {
            error!("single env's cache gc error! root={}, {}", new_id, e);
        }

        self.cache.commit().await?;

        info!(
            "single op_env update root success! sid={}, root=={}",
            self.sid, new_id
        );
        Ok(new_id)
    }

    pub async fn update(&self) -> BuckyResult<ObjectId> {
        self.update_root(false).await
    }

    pub async fn commit(self) -> BuckyResult<ObjectId> {
        self.update_root(true).await
    }

    pub fn abort(self) -> BuckyResult<()> {
        info!("will abort single_op_env: sid={}", self.sid);
        self.cache.abort();

        Ok(())
    }
}

#[derive(Clone)]
pub struct ObjectMapSingleOpEnvRef(Arc<ObjectMapSingleOpEnv>);

impl ObjectMapSingleOpEnvRef {
    pub fn new(env: ObjectMapSingleOpEnv) -> Self {
        Self(Arc::new(env))
    }

    fn into_raw(self) -> BuckyResult<ObjectMapSingleOpEnv> {
        let sid = self.sid();
        let env = Arc::try_unwrap(self.0).map_err(|this| {
            let msg = format!(
                "single_op_env's ref_count is more than one! sid={}, ref={}",
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

impl std::ops::Deref for ObjectMapSingleOpEnvRef {
    type Target = Arc<ObjectMapSingleOpEnv>;
    fn deref(&self) -> &Arc<ObjectMapSingleOpEnv> {
        &self.0
    }
}
