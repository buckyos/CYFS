use super::cache::*;
use super::check::*;
use super::iterator::*;
use super::object_map::*;
use super::op::*;
use crate::*;

use std::sync::{Arc, Mutex};

// 基于路径管理的ObjectMap集合，共享同一个root，每级子路径对应一个ObjectMap
pub struct ObjectMapPath {
    root: Arc<Mutex<ObjectId>>,
    obj_map_cache: ObjectMapOpEnvCacheRef,

    // 用以暂存所有写入操作
    write_ops: Option<ObjectMapOpList>,
}

struct ObjectMapPathSeg {
    obj_map: ObjectMap,
    seg: Option<String>,
}

impl std::fmt::Debug for ObjectMapPathSeg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?},{:?}]", self.seg, self.obj_map.cached_object_id())
    }
}

impl ObjectMapPath {
    pub fn new(
        root: ObjectId,
        obj_map_cache: ObjectMapOpEnvCacheRef,
        enable_transaction: bool,
    ) -> Self {
        Self {
            root: Arc::new(Mutex::new(root)),
            obj_map_cache,
            write_ops: if enable_transaction {
                Some(ObjectMapOpList::new())
            } else {
                None
            },
        }
    }

    // 获取当前的root
    pub fn root(&self) -> ObjectId {
        self.root.lock().unwrap().clone()
    }

    pub fn update_root(&self, root_id: ObjectId, prev_id: &ObjectId) -> BuckyResult<()> {
        let mut root = self.root.lock().unwrap();
        if *root != *prev_id {
            let msg = format!(
                "update root but unmatch! current={}, prev={}, new={}",
                *root, prev_id, root_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        info!("objectmap path root updated! {} -> {}", *root, root_id);
        *root = root_id;
        Ok(())
    }

    async fn get_root(&self) -> BuckyResult<ObjectMapRef> {
        let root_id = self.root();
        let ret = self.obj_map_cache.get_object_map(&root_id).await?;
        if ret.is_none() {
            let msg = format!("load root object but not found! id={}", root_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        Ok(ret.unwrap())
    }

    /*
    /a/b/ -> /a/b
    / -> /
    /a/b?arg=xxx -> /a/b
    */
    fn fix_path(path: &str) -> BuckyResult<&str> {
        let path = path.trim();
        if path == "/" {
            return Ok(path);
        }

        // Remove the query params
        let path = match path.rsplit_once('?') {
            Some((path, _)) => path,
            None => path,
        };

        // The / at the end needs to be removed
        let path_ret = path.trim_end_matches("/");
        if !path_ret.starts_with("/") {
            let msg = format!("invalid objectmap path format! path={}", path);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(path_ret)
    }

    // 获取path对应的obj_map叶子节点
    async fn get_object_map(&self, path: &str) -> BuckyResult<Option<ObjectMapRef>> {
        let mut current = self.get_root().await?;

        let path = Self::fix_path(path)?;
        // Check if is root path
        if path == "/" {
            return Ok(Some(current));
        }

        // 依次获取每级子路径
        let parts = path.split("/").skip(1);
        for part in parts {
            ObjectMapChecker::check_key_value(part)?;

            let sub = current
                .lock()
                .await
                .get_or_create_child_object_map(
                    &self.obj_map_cache,
                    part,
                    ObjectMapSimpleContentType::Map,
                    ObjectMapCreateStrategy::NotCreate,
                    None,
                )
                .await
                .map_err(|e| {
                    let msg = format!(
                        "get object by path error! path={}, part={}, {}",
                        path, part, e
                    );
                    error!("{}", msg);
                    BuckyError::new(e.code(), msg)
                })?;

            if sub.is_none() {
                let msg = format!(
                    "get object by path but not found! path={}, part={}",
                    path, part
                );
                warn!("{}", msg);
                return Ok(None);
            }

            current = sub.unwrap();
            debug!(
                "get objectmap path seg: {}={:?}",
                part,
                current.lock().await.cached_object_id()
            );
        }

        Ok(Some(current))
    }

    // 以/开头的路径
    async fn create_object_map(
        &self,
        path: &str,
        content_type: ObjectMapSimpleContentType,
        auto_create: ObjectMapCreateStrategy,
    ) -> BuckyResult<Option<Vec<ObjectMapPathSeg>>> {
        let root = self.get_root().await?;
        let current = root.lock().await.clone();

        let path = Self::fix_path(path)?;

        let root_seg = ObjectMapPathSeg {
            obj_map: current,
            seg: None,
        };

        let mut obj_list = vec![root_seg];

        // 判断是不是root
        if path == "/" {
            trace!("object map path list: path={}, list={:?}", path, obj_list);
            return Ok(Some(obj_list));
        }

        // 依次获取每级子路径
        let parts: Vec<&str> = path.split("/").skip(1).collect();
        for (index, &part) in parts.iter().enumerate() {
            ObjectMapChecker::check_key_value(part)?;

            let is_last_part = index == parts.len() - 1;
            // 最后一级使用目标类型， 中间子目录统一使用map
            let content_type = if is_last_part {
                content_type.clone()
            } else {
                ObjectMapSimpleContentType::Map
            };

            let create_strategy = match auto_create {
                ObjectMapCreateStrategy::CreateIfNotExists => {
                    ObjectMapCreateStrategy::CreateIfNotExists
                }
                ObjectMapCreateStrategy::NotCreate => ObjectMapCreateStrategy::NotCreate,
                ObjectMapCreateStrategy::CreateNew => {
                    // only use createNew for the last seg
                    if is_last_part {
                        ObjectMapCreateStrategy::CreateNew
                    } else {
                        ObjectMapCreateStrategy::CreateIfNotExists
                    }
                }
            };

            let sub = obj_list
                .last_mut()
                .unwrap()
                .obj_map
                .get_or_create_child_object_map(&self.obj_map_cache, part, content_type, create_strategy, None)
                .await
                .map_err(|e| {
                    let msg = format!(
                        "get or create object by path error! path={}, part={}, create_strategy={:?}, {}",
                        path, part, create_strategy, e
                    );
                    error!("{}", msg);
                    BuckyError::new(e.code(), msg)
                })?;

            if sub.is_none() {
                let msg = format!(
                    "get object by path but not found! path={}, part={}",
                    path, part
                );
                warn!("{}", msg);
                return Ok(None);
            }

            // 可能涉及到修改操作,所以路径上的objectmap都clone一份
            let current = sub.unwrap().lock().await.clone();
            let current_seq = ObjectMapPathSeg {
                obj_map: current,
                seg: Some(part.to_owned()),
            };

            obj_list.push(current_seq);
        }

        debug!("object map path list: path={}, list={:?}", path, obj_list);

        Ok(Some(obj_list))
    }

    async fn update_path_obj_map_list(
        &self,
        mut obj_map_list: Vec<ObjectMapPathSeg>,
    ) -> BuckyResult<Vec<(ObjectMap, ObjectId)>> {
        assert!(!obj_map_list.is_empty());

        let mut current_obj_map = obj_map_list.pop().unwrap();
        let mut new_obj_map_list = vec![];

        // 更新路径上的所有obj_map
        loop {
            // 刷新当前obj_map的id
            let prev_id = current_obj_map.obj_map.cached_object_id().unwrap();
            let current_id = current_obj_map.obj_map.flush_id();
            assert_ne!(prev_id, current_id);

            trace!(
                "update objectmap path seg: seg={:?}, {} -> {}",
                current_obj_map.seg, prev_id, current_id
            );

            // 更新此段的obj_map(id发生了变化)
            new_obj_map_list.push((current_obj_map.obj_map, prev_id.clone()));

            if obj_map_list.is_empty() {
                break;
            }

            // 如果存在父一级，那么需要更新到父级obj_map
            let seg = current_obj_map.seg.unwrap();
            assert!(seg.len() > 0);

            let mut parent_obj_map = obj_map_list.pop().unwrap();
            parent_obj_map
                .obj_map
                .set_with_key(
                    &self.obj_map_cache,
                    &seg,
                    &current_id,
                    &Some(prev_id),
                    false,
                )
                .await
                .map_err(|e| e)?;

            current_obj_map = parent_obj_map;
        }

        Ok(new_obj_map_list)
    }

    // 新的path刷新到缓存，并更新root
    fn flush_path_obj_map_list(&self, obj_map_list: Vec<(ObjectMap, ObjectId)>) -> BuckyResult<()> {
        let count = obj_map_list.len();

        // 从叶子节点向根节点依次更新，最后更新root
        for (index, (obj_map, prev_id)) in obj_map_list.into_iter().enumerate() {
            // 最新的map必须已经计算过id了
            let current_id = obj_map.cached_object_id().unwrap();
            assert_ne!(current_id, prev_id);

            self.obj_map_cache
                .put_object_map(&current_id, obj_map, None)?;

            if index + 1 == count {
                self.update_root(current_id, &prev_id)?;
            }

            // TODO 如果之前的object在pending区，那么尝试移除
            // 这里不能简单的移除，因为可能别的path操作里面长生的新路径存在相同对象id，会引用，这里移除会导致后续的查找失败
            // 需要依赖统一的GC逻辑
            // remove_list.insert(prev_id);
        }

        Ok(())
    }

    pub async fn metadata(&self, path: &str) -> BuckyResult<ObjectMapMetaData> {
        let ret = self.get_object_map(path).await?;
        if ret.is_none() {
            let msg = format!("get value from path but objectmap not found! path={}", path);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let ret = ret.unwrap();
        let obj = ret.lock().await;
        Ok(obj.metadata())
    }

    pub async fn list(&self, path: &str) -> BuckyResult<ObjectMapContentList> {
        let ret = self.get_object_map(path).await?;
        if ret.is_none() {
            let msg = format!("get value from path but objectmap not found! path={}", path);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let item = ret.unwrap();
        let obj = item.lock().await;
        let mut list = ObjectMapContentList::new(obj.count() as usize);
        obj.list(&self.obj_map_cache, &mut list).await?;
        Ok(list)
    }

    pub fn parse_path_allow_empty_key(full_path: &str) -> BuckyResult<(&str, &str)> {
        let full_path = Self::fix_path(full_path)?;

        if full_path == "/" {
            return Ok((full_path, ""));
        }

        let mut path_segs: Vec<&str> = full_path.split("/").collect();
        if path_segs.len() < 2 {
            let msg = format!("invalid objectmap full path: {}", full_path);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let key = path_segs.pop().unwrap();
        let trim_len = if path_segs.len() > 1 {
            key.len() + 1
        } else {
            key.len()
        };

        let path = &full_path[..(full_path.len() - trim_len)];
        if path.len() == 0 {
            let msg = format!("invalid objectmap full path: {}", full_path);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok((path, key))
    }

    // map methods with full_path
    // key should not been empty string
    // 用来解析全路径，提取path和key
    /*
    /a -> / + a
    /a/b/ -> /a + b
    / -> Err
    */
    pub fn parse_full_path(full_path: &str) -> BuckyResult<(&str, &str)> {
        let (path, key) = Self::parse_path_allow_empty_key(full_path)?;

        let full_path = Self::fix_path(full_path)?;

        if key.len() == 0 {
            let msg = format!("invalid objectmap full path: {}", full_path);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok((path, key))
    }

    pub async fn get_by_path(&self, full_path: &str) -> BuckyResult<Option<ObjectId>> {
        let (path, key) = Self::parse_path_allow_empty_key(full_path)?;

        self.get_by_key(path, key).await
    }

    pub async fn create_new_with_path(
        &self,
        full_path: &str,
        content_type: ObjectMapSimpleContentType,
    ) -> BuckyResult<()> {
        let (path, key) = Self::parse_full_path(full_path)?;

        self.create_new(path, key, content_type).await
    }

    pub async fn insert_with_path(&self, full_path: &str, value: &ObjectId) -> BuckyResult<()> {
        let (path, key) = Self::parse_full_path(full_path)?;

        self.insert_with_key(path, key, value).await
    }

    pub async fn set_with_path(
        &self,
        full_path: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let (path, key) = Self::parse_full_path(full_path)?;

        self.set_with_key(path, key, value, prev_value, auto_insert)
            .await
    }

    pub async fn remove_with_path(
        &self,
        full_path: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        let (path, key) = Self::parse_full_path(full_path)?;

        self.remove_with_key(path, key, prev_value).await
    }

    // map methods
    pub async fn create_new(
        &self,
        path: &str,
        key: &str,
        content_type: ObjectMapSimpleContentType,
    ) -> BuckyResult<()> {
        // 创建事务
        let param = CreateNewParam {
            key: key.to_owned(),
            content_type,
        };
        let op_data = CreateNewOpData {
            path: path.to_owned(),
            param,
            state: None,
        };

        let ret = self.create_new_op(&op_data).await?;

        // insert不需要保存状态，只要插入成功，那么状态就认为是一致的

        if let Some(write_ops) = &self.write_ops {
            write_ops.append_op(ObjectMapWriteOp::CreateNew(op_data));
        }

        Ok(ret)
    }

    async fn create_new_op(&self, op_data: &CreateNewOpData) -> BuckyResult<()> {
        // 首先获取路径上的所有ObjectMap(空目录自动创建)
        let ret = self
            .create_object_map(
                &op_data.path,
                ObjectMapSimpleContentType::Map,
                ObjectMapCreateStrategy::CreateIfNotExists,
            )
            .await?;
        let mut obj_map_list = ret.unwrap();
        assert!(obj_map_list.len() > 0);

        // create_new不需要保存旧值，因为如果存在旧值，那么会直接失败；只有为空才可以创建成功
        obj_map_list
            .last_mut()
            .unwrap()
            .obj_map
            .get_or_create_child_object_map(
                &self.obj_map_cache,
                &op_data.param.key,
                op_data.param.content_type,
                ObjectMapCreateStrategy::CreateNew,
                None,
            )
            .await?;

        let list = self.update_path_obj_map_list(obj_map_list).await?;
        self.flush_path_obj_map_list(list)?;

        Ok(())
    }

    pub async fn get_by_key(&self, path: &str, key: &str) -> BuckyResult<Option<ObjectId>> {
        let ret = self.get_object_map(path).await?;
        if ret.is_none() {
            info!(
                "get value from path but objectmap not found! path={}, key={}",
                path, key
            );
            return Ok(None);
        }

        // without key, return the path last node
        if key.len() == 0 {
            let obj_map = ret.as_ref().unwrap().lock().await;
            return Ok(obj_map.cached_object_id());
        }

        let ret = ret.unwrap();
        let obj_map = ret.lock().await;
        obj_map.get_by_key(&self.obj_map_cache, key).await
    }

    pub async fn insert_with_key(
        &self,
        path: &str,
        key: &str,
        value: &ObjectId,
    ) -> BuckyResult<()> {
        // 创建事务
        let param = InsertWithKeyParam {
            key: key.to_owned(),
            value: value.to_owned(),
        };
        let op_data = InsertWithKeyOpData {
            path: path.to_owned(),
            param,
            state: None,
        };

        let ret = self.insert_with_key_op(&op_data).await?;

        // insert不需要保存状态，只要插入成功，那么状态就认为是一致的

        if let Some(write_ops) = &self.write_ops {
            write_ops.append_op(ObjectMapWriteOp::InsertWithKey(op_data));
        }

        Ok(ret)
    }

    async fn insert_with_key_op(&self, op_data: &InsertWithKeyOpData) -> BuckyResult<()> {
        // 首先获取路径上的所有ObjectMap(空目录自动创建)
        let ret = self
            .create_object_map(
                &op_data.path,
                ObjectMapSimpleContentType::Map,
                ObjectMapCreateStrategy::CreateIfNotExists,
            )
            .await?;
        let mut obj_map_list = ret.unwrap();
        assert!(obj_map_list.len() > 0);

        // insert_with_key不需要保存旧值，因为如果存在旧值，那么会直接失败；只有为空才可以插入成功
        obj_map_list
            .last_mut()
            .unwrap()
            .obj_map
            .insert_with_key(
                &self.obj_map_cache,
                &op_data.param.key,
                &op_data.param.value,
            )
            .await?;

        let list = self.update_path_obj_map_list(obj_map_list).await?;
        self.flush_path_obj_map_list(list)?;

        Ok(())
    }

    pub async fn set_with_key(
        &self,
        path: &str,
        key: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        // 创建事务
        let param = SetWithKeyParam {
            key: key.to_owned(),
            value: value.to_owned(),
            prev_value: prev_value.to_owned(),
            auto_insert,
        };

        let mut op_data = SetWithKeyOpData {
            path: path.to_owned(),
            param,
            state: None,
        };

        let ret = self.set_with_key_op(&op_data).await?;

        // 保存状态
        if let Some(write_ops) = &self.write_ops {
            let state = ObjectMapKeyState { value: ret.clone() };
            op_data.state = Some(state);

            write_ops.append_op(ObjectMapWriteOp::SetWithKey(op_data));
        }

        Ok(ret)
    }

    async fn set_with_key_op(&self, op_data: &SetWithKeyOpData) -> BuckyResult<Option<ObjectId>> {
        // 首先获取路径上的所有ObjectMap(空目录自动创建)

        let create_strategy = if op_data.param.auto_insert {
            ObjectMapCreateStrategy::CreateIfNotExists
        } else {
            ObjectMapCreateStrategy::NotCreate
        };

        let obj_map_list = self
            .create_object_map(
                &op_data.path,
                ObjectMapSimpleContentType::Map,
                create_strategy,
            )
            .await?;
        if obj_map_list.is_none() {
            // 如果auto_insert=false，并且路径不存在，那么直接返回Err(NotFound)
            let msg = format!(
                "set_with_key but path not found! path={}, value={}",
                op_data.path, op_data.param.value,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let mut obj_map_list = obj_map_list.unwrap();
        assert!(obj_map_list.len() > 0);

        // set_with_key存在以下几种情况:
        // 1. 当前slot为空，auto_insert=false，那么直接返回Err(NotFound)
        // 2. 当前slot为空，auto_insert=true，那么操作成功，返回Ok(None)
        // 3. 当前slot不为空，prev_value=None, 那么操作成功，返回Ok(prev_value)
        // 4. 当前slot不为空, prev_value!=None, 那么只有当前value和prev_value匹配，才成功，并且返回当前值；否则返回Err(Unmatch)
        let ret = obj_map_list
            .last_mut()
            .unwrap()
            .obj_map
            .set_with_key(
                &self.obj_map_cache,
                &op_data.param.key,
                &op_data.param.value,
                &op_data.param.prev_value,
                op_data.param.auto_insert,
            )
            .await?;

        // 判断状态是否一致
        if let Some(state) = &op_data.state {
            if ret != state.value {
                let msg = format!(
                    "set_with_key with path commit but state conflict! op_data={:?}, ret={:?}",
                    op_data, ret,
                );
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Conflict, msg));
            }
        }

        if ret != Some(op_data.param.value) {
            let list = self.update_path_obj_map_list(obj_map_list).await?;
            self.flush_path_obj_map_list(list)?;
        }

        Ok(ret)
    }

    pub async fn remove_with_key(
        &self,
        path: &str,
        key: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        // 创建事务
        let param = RemoveWithKeyParam {
            key: key.to_owned(),
            prev_value: prev_value.to_owned(),
        };
        let mut op_data = RemoveWithKeyOpData {
            path: path.to_owned(),
            param,
            state: None,
        };

        let ret = self.remove_with_key_op(&op_data).await?;

        // 保存状态
        if let Some(write_ops) = &self.write_ops {
            let state = ObjectMapKeyState { value: ret.clone() };
            op_data.state = Some(state);

            write_ops.append_op(ObjectMapWriteOp::RemoveWithKey(op_data));
        }

        Ok(ret)
    }

    async fn remove_with_key_op(
        &self,
        op_data: &RemoveWithKeyOpData,
    ) -> BuckyResult<Option<ObjectId>> {
        let (ret, obj_map_list) = loop {
            let ret = self
                .create_object_map(
                    &op_data.path,
                    ObjectMapSimpleContentType::Map,
                    ObjectMapCreateStrategy::NotCreate,
                )
                .await?;

            // 所在目录不存在，那么直接返回不存在即可
            if ret.is_none() {
                debug!(
                    "objectmap path remove_with_key but path not found! root={}, path={}, key={}",
                    self.root(),
                    op_data.path,
                    op_data.param.key,
                );

                break (None, None);
            }

            let mut obj_map_list = ret.unwrap();
            assert!(obj_map_list.len() > 0);

            // 发起真正的remove操作
            let ret = obj_map_list
                .last_mut()
                .unwrap()
                .obj_map
                .remove_with_key(
                    &self.obj_map_cache,
                    &op_data.param.key,
                    &op_data.param.prev_value,
                )
                .await?;

            info!(
                "objectmap path remove_with_key success! root={}, path={}, key={}, value={:?}",
                self.root(),
                op_data.path,
                op_data.param.key,
                ret
            );
            break (ret, Some(obj_map_list));
        };

        // 判断状态是否一致
        if let Some(state) = &op_data.state {
            if ret != state.value {
                let msg = format!(
                    "remove_with_key from path commit but state conflict! op_data={:?}, ret={:?}",
                    op_data, ret,
                );
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Conflict, msg));
            }
        }

        if ret.is_none() {
            return Ok(None);
        }

        // 内容改变了，需要更新整个路径
        let list = self.update_path_obj_map_list(obj_map_list.unwrap()).await?;
        self.flush_path_obj_map_list(list)?;

        Ok(ret)
    }

    // set methods
    pub async fn contains(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        let ret = self.get_object_map(path).await?;

        if ret.is_none() {
            let msg = format!(
                "contains from path but objectmap not found! path={}, value={}",
                path, object_id,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let ret = ret.unwrap();
        let obj_map = ret.lock().await;
        obj_map.contains(&self.obj_map_cache, object_id).await
    }

    pub async fn insert(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        // 创建事务
        let param = InsertParam {
            value: object_id.to_owned(),
        };
        let mut op_data = InsertOpData {
            path: path.to_owned(),
            param,
            state: None,
        };

        let ret = self.insert_op(&op_data).await?;

        // 保存现有状态
        if let Some(write_ops) = &self.write_ops {
            op_data.state = Some(ret);

            write_ops.append_op(ObjectMapWriteOp::Insert(op_data));
        }

        Ok(ret)
    }

    async fn insert_op(&self, op_data: &InsertOpData) -> BuckyResult<bool> {
        let obj_map_list = self
            .create_object_map(
                &op_data.path,
                ObjectMapSimpleContentType::Set,
                ObjectMapCreateStrategy::CreateIfNotExists,
            )
            .await?;

        let mut obj_map_list = obj_map_list.unwrap();
        assert!(obj_map_list.len() > 0);

        // 发起真正的insert操作
        let ret = obj_map_list
            .last_mut()
            .unwrap()
            .obj_map
            .insert(&self.obj_map_cache, &op_data.param.value)
            .await?;
        // 如果事务是带状态的，那么需要校验一次状态
        if let Some(state) = &op_data.state {
            if *state != ret {
                let msg = format!(
                    "insert to path commit but state conflict! op_data={:?}",
                    op_data,
                );
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Conflict, msg));
            }
        }

        // 值不存在，插入成功，需要更新路径
        if ret {
            // 内容改变了，需要更新整个路径
            let list = self.update_path_obj_map_list(obj_map_list).await?;
            self.flush_path_obj_map_list(list)?;
        }

        Ok(ret)
    }

    pub async fn remove(&self, path: &str, object_id: &ObjectId) -> BuckyResult<bool> {
        // 创建事务
        let param = RemoveParam {
            value: object_id.to_owned(),
        };
        let mut op_data = RemoveOpData {
            path: path.to_owned(),
            param,
            state: None,
        };

        let ret = self.remove_op(&op_data).await?;

        // 保存状态
        if let Some(write_ops) = &self.write_ops {
            op_data.state = Some(ret);

            write_ops.append_op(ObjectMapWriteOp::Remove(op_data));
        }

        Ok(ret)
    }

    async fn remove_op(&self, op_data: &RemoveOpData) -> BuckyResult<bool> {
        let ret = self
            .create_object_map(
                &op_data.path,
                ObjectMapSimpleContentType::Set,
                ObjectMapCreateStrategy::NotCreate,
            )
            .await?;

        // 所在目录不存在，那么直接返回错误
        if ret.is_none() {
            let msg = format!(
                "remove but path not found! path={}, value={}",
                op_data.path, op_data.param.value,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let mut obj_map_list = ret.unwrap();
        assert!(obj_map_list.len() > 0);

        // 发起真正的remove操作
        let ret = obj_map_list
            .last_mut()
            .unwrap()
            .obj_map
            .remove(&self.obj_map_cache, &op_data.param.value)
            .await?;

        // 如果事务是带状态的，那么需要校验一次状态
        if let Some(state) = &op_data.state {
            if *state != ret {
                let msg = format!(
                    "remove from path commit but state conflict! op_data={:?}",
                    op_data,
                );
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Conflict, msg));
            }
        }

        if ret {
            // 内容改变了，需要更新整个路径
            let list = self.update_path_obj_map_list(obj_map_list).await?;
            self.flush_path_obj_map_list(list)?;
        }

        Ok(ret)
    }

    pub fn clear_op_list(&self) {
        if let Some(write_ops) = &self.write_ops {
            let _ = write_ops.fetch_all();
        }
    }

    // 提交操作列表，用以实现事务的commit
    pub async fn commit_op_list(&self) -> BuckyResult<()> {
        let op_list = self.write_ops.as_ref().unwrap().fetch_all();

        for op_data in op_list {
            self.commit_op(op_data).await?;
        }

        Ok(())
    }

    async fn commit_op(&self, op: ObjectMapWriteOp) -> BuckyResult<()> {
        match op {
            ObjectMapWriteOp::CreateNew(op_data) => {
                self.create_new_op(&op_data).await?;
            }
            ObjectMapWriteOp::InsertWithKey(op_data) => {
                self.insert_with_key_op(&op_data).await?;
            }
            ObjectMapWriteOp::SetWithKey(op_data) => {
                self.set_with_key_op(&op_data).await?;
            }
            ObjectMapWriteOp::RemoveWithKey(op_data) => {
                self.remove_with_key_op(&op_data).await?;
            }

            ObjectMapWriteOp::Insert(op_data) => {
                self.insert_op(&op_data).await?;
            }
            ObjectMapWriteOp::Remove(op_data) => {
                self.remove_op(&op_data).await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test_path {
    use super::super::cache::*;
    use super::super::path_iterator::*;
    use super::*;

    use std::str::FromStr;

    async fn dump_path(item: &ObjectMapPath, path: &str) {
        let list = item.list(path).await.unwrap();
        info!("dump path={} as follows:", path);
        info!("{}", list);
    }

    async fn test_path1(path: &ObjectMapPath) {
        let x1_value = ObjectId::from_str("5aSixgPg3hDa1oU9eAtRcKTyVKg5X2bVXWPVhk3U5c7G").unwrap();
        let x1_value2 = ObjectId::from_str("5aSixgPCivmQfASRbjAvBiwgxhU8LrNtYtC2D6Lis2NQ").unwrap();

        path.insert_with_key("/", "x1", &x1_value).await.unwrap();

        let ret = path.get_by_key("/a/b/c", "x1").await.unwrap();
        assert!(ret.is_none());

        let ret = path.get_by_path("/a/b/c/x1").await.unwrap();
        assert!(ret.is_none());

        path.insert_with_key("/a/b/c", "x1", &x1_value)
            .await
            .unwrap();
        let ret = path.insert_with_path("/a/b/c/x1", &x1_value).await;
        let e = ret.unwrap_err();
        assert_eq!(e.code(), BuckyErrorCode::AlreadyExists);

        let ret = path.get_by_key("/a/b/c", "x1").await.unwrap();
        assert_eq!(ret, Some(x1_value));
        let ret = path.get_by_path("/a/b/c/x1").await.unwrap();
        assert_eq!(ret, Some(x1_value));

        dump_path(path, "/").await;
        dump_path(path, "/a").await;
        dump_path(path, "/a/b").await;
        dump_path(path, "/a/b/c").await;
        let ret = path.get_by_key("/a/b/c", "x1").await.unwrap();
        assert_eq!(ret, Some(x1_value));

        // 插入已经存在的key，返回错误
        let ret = path.insert_with_key("/a/b/c", "x1", &x1_value).await;
        let err = ret.unwrap_err();
        assert_eq!(err.code(), BuckyErrorCode::AlreadyExists);

        // 测试set_with_key
        let ret = path
            .set_with_key("/a/b/c", "x1", &x1_value2, &Some(x1_value2), false)
            .await;
        assert!(ret.is_err());
        let err = ret.unwrap_err();
        assert_eq!(err.code(), BuckyErrorCode::Unmatch);

        let ret = path
            .set_with_key("/a/b/c", "x1", &x1_value2, &Some(x1_value), false)
            .await
            .unwrap();
        assert_eq!(ret, Some(x1_value));

        // 测试删除
        let ret = path.remove_with_key("/a/b/c", "x1", &Some(x1_value)).await;
        assert!(ret.is_err());
        let err = ret.unwrap_err();
        assert_eq!(err.code(), BuckyErrorCode::Unmatch);

        let ret = path.remove_with_key("/a/b/c", "x1", &None).await.unwrap();
        assert_eq!(ret, Some(x1_value2));

        // 再次测试set_with_key
        let ret = path
            .set_with_key("/a/b/c", "x1", &x1_value2, &None, false)
            .await;
        assert!(ret.is_err());
        let err = ret.unwrap_err();
        assert_eq!(err.code(), BuckyErrorCode::NotFound);

        // 自动插入x1
        let ret = path
            .set_with_key("/a/b/c", "x1", &x1_value2, &None, true)
            .await
            .unwrap();
        assert_eq!(ret, None);

        let ret = path.get_by_key("/a/b/c", "x1").await.unwrap();
        assert_eq!(ret, Some(x1_value2));

        let ret = path.remove_with_key("/a/b/c", "x1", &None).await.unwrap();
        assert_eq!(ret, Some(x1_value2));

        let ret = path.get_by_key("/a/b/c", "x1").await.unwrap();
        assert!(ret.is_none());

        let ret = path.get_by_key("/a/b", "c").await.unwrap();
        assert!(ret.is_some());
        let c_id = ret.unwrap();
        info!("/a/b/c={}", c_id);

        dump_path(path, "/").await;
        dump_path(path, "/a").await;
        dump_path(path, "/a/b").await;
        dump_path(path, "/a/b/c").await;

        let ret = path.remove_with_key("/a/b", "c", &None).await.unwrap();
        assert_eq!(ret, Some(c_id));

        let ret = path.get_by_key("/a/b/c", "x1").await.unwrap();
        assert!(ret.is_none());

        let ret = path.get_by_key("/a/b", "c").await.unwrap();
        assert!(ret.is_none());

        let ret = path.get_by_key("/a/b/c", "x1").await.unwrap();
        assert!(ret.is_none());

        let ret = path.get_by_path("/").await.unwrap();
        assert!(ret.is_some());

        path.create_new("/a/b", "c", ObjectMapSimpleContentType::Set)
            .await
            .unwrap();
        if let Err(e) = path
            .create_new("/a/b", "c", ObjectMapSimpleContentType::Set)
            .await
        {
            assert!(e.code() == BuckyErrorCode::AlreadyExists);
        } else {
            unreachable!();
        }
        if let Err(e) = path
            .create_new("/a/b", "c", ObjectMapSimpleContentType::Set)
            .await
        {
            assert!(e.code() == BuckyErrorCode::AlreadyExists);
        } else {
            unreachable!();
        }

        let ret = path.get_by_key("/a/b", "c").await.unwrap();
        assert!(ret.is_some());
    }

    async fn test_path() {
        let noc = ObjectMapMemoryNOCCache::new();
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(None, noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        // 创建一个空的objectmap作为root
        let owner = ObjectId::default();
        let root = ObjectMap::new(
            ObjectMapSimpleContentType::Map,
            Some(owner.clone()),
            Some(owner.clone()),
        )
        .no_create_time()
        .build();
        let root_id = root.flush_id();
        cache.put_object_map(&root_id, root, None).unwrap();
        info!("new root: {}", root_id);

        let path = ObjectMapPath::new(root_id.clone(), cache.clone(), true);
        test_path1(&path).await;

        let opt = ObjectMapPathIteratorOption::new(true, true);
        let root = path.root();
        let root_obj = cache.get_object_map(&root).await.unwrap();
        let mut it =
            ObjectMapPathIterator::new(root_obj.unwrap(), cache.clone(), opt.clone()).await;
        while !it.is_end() {
            let list = it.next(5).await.unwrap();
            info!("list: {} {:?}", 1, list.list);
        }

        let root_id = path.root();
        info!("result root: {}", root_id);

        cache.gc(false, &root_id).await.unwrap();

        let root_obj = cache.get_object_map(&root_id).await.unwrap();
        let mut it =
            ObjectMapPathIterator::new(root_obj.unwrap(), cache.clone(), opt.clone()).await;
        while !it.is_end() {
            let list = it.next(5).await.unwrap();
            info!("list: {} {:?}", 1, list.list);
        }
    }

    #[test]
    fn test_full_path() {
        ObjectMapPath::parse_full_path("/").unwrap_err();
        let (path, key) = ObjectMapPath::parse_full_path("/a").unwrap();
        assert_eq!(path, "/");
        assert_eq!(key, "a");

        let (path, key) = ObjectMapPath::parse_full_path("/a/").unwrap();
        assert_eq!(path, "/");
        assert_eq!(key, "a");

        let (path, key) = ObjectMapPath::parse_full_path("/a/b").unwrap();
        assert_eq!(path, "/a");
        assert_eq!(key, "b");

        let (path, key) = ObjectMapPath::parse_full_path("/eeee/eeee").unwrap();
        assert_eq!(path, "/eeee");
        assert_eq!(key, "eeee");

        let (path, key) = ObjectMapPath::parse_full_path("/eeee/eeee/").unwrap();
        assert_eq!(path, "/eeee");
        assert_eq!(key, "eeee");
    }

    #[test]
    fn test() {
        crate::init_simple_log("test-object-map-path", Some("debug"));
        test_full_path();
        async_std::task::block_on(async move {
            test_path().await;
        });
    }
}
