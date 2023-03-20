use super::cache::*;
use super::iterator::*;
use super::object_map::*;
use crate::*;

use std::collections::VecDeque;

/*
pub const OBJECT_MAP_DIFF_ADDED_KEY: &str = "added";
pub const OBJECT_MAP_DIFF_ALTERED_KEY: &str = "altered";
pub const OBJECT_MAP_DIFF_PREV_KEY: &str = "prev";
pub const OBJECT_MAP_DIFF_REMOVED_KEY: &str = "removed";
pub const OBJECT_MAP_DIFF_DIFF_KEY: &str = "diff";
*/

enum ObjectMapPendingDiff {
    SubAlter((ObjectId, ObjectId)),
    Alter((ObjectId, ObjectId)),
    Add(ObjectId),
    Remove(ObjectId),
}

enum ObjectMapPendingResult {
    Map((ObjectMapDiffAction, String, Option<ObjectId>, ObjectId)),
    Set((ObjectMapDiffAction, ObjectId)),
}
/*
同类型的ObjectMap才可以Diff
不同类型的ObjectMap，必须在上一级完成diff
*/

pub struct ObjectMapDiff {
    owner: Option<ObjectId>,
    dec_id: Option<ObjectId>,

    // 只有两个类型一致的ObjectMap才可以diff操作
    content_type: ObjectMapSimpleContentType,

    // 是否递归展开child diff
    expand_altered: bool,

    cache: ObjectMapOpEnvCacheRef,

    // diff结果存放的ObjectMap
    // Map -> DiffMap, Set -> DiffSet
    result: Option<ObjectMap>,

    // 等待被异步处理的diff队列
    pending_diffs: VecDeque<ObjectMapPendingDiff>,

    // 等待被添加到结果ObjectMap的对象列表
    pending_results: VecDeque<ObjectMapPendingResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectMapDiffAction {
    Add,
    Alter,
    Remove,
}

// 两个ObjectMap的叶子节点的最终的diff，一定会落到下面两种形式:
// 一、两个SimpleContent的diff
// 二、一个SimpleContent和一个HubContent的diff

impl ObjectMapDiff {
    pub fn new(
        owner: Option<ObjectId>,
        dec_id: Option<ObjectId>,
        cache: ObjectMapOpEnvCacheRef,
        content_type: ObjectMapSimpleContentType,
        expand_altered: bool,
    ) -> Self {
        let result = ObjectMap::new(
            content_type.get_diff_type().unwrap(),
            owner.clone(),
            dec_id.clone(),
        )
        .build();

        Self {
            owner,
            dec_id,
            cache,
            content_type,
            expand_altered,

            result: Some(result),

            pending_diffs: VecDeque::new(),
            pending_results: VecDeque::new(),
        }
    }

    // 计算两个对象的diff，id不能相同
    #[async_recursion::async_recursion]
    pub async fn diff_objects(
        cache: &ObjectMapOpEnvCacheRef,
        prev: &ObjectId,
        next: &ObjectId,
        expand_altered: bool,
    ) -> BuckyResult<ObjectId> {
        if *prev == *next {
            let msg = format!("diff object but with id the same! id={}", prev);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let prev_obj = cache.get_object_map(prev).await?;
        if prev_obj.is_none() {
            let msg = format!("diff object but not found! prev={}", prev);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let next_obj = cache.get_object_map(next).await?;
        if next_obj.is_none() {
            let msg = format!("diff object but not found! altered={}", next);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let prev_obj = prev_obj.unwrap();
        let next_obj = next_obj.unwrap();

        let mut diff = {
            let prev_obj = prev_obj.lock().await;
            let next_obj = next_obj.lock().await;

            if prev_obj.content_type() != next_obj.content_type() {
                let msg = format!(
                    "diff object but content_type not match! prev={}, next={}",
                    prev, next
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }

            ObjectMapDiff::new(
                prev_obj.desc().owner().to_owned(),
                prev_obj.desc().dec_id().to_owned(),
                cache.clone(),
                prev_obj.content_type(),
                expand_altered,
            )
        };

        // 把根ObjectMap放置到pending列表
        diff.pend_async_sub_alter(&prev, &next);

        // 计算diff
        diff.calc_diff().await?;

        // 转换成diff_object
        diff.into_object_map().await
    }

    async fn into_object_map(&mut self) -> BuckyResult<ObjectId> {
        let diff_object = self.result.take().unwrap();

        let id = diff_object.flush_id();
        self.cache.put_object_map(&id, diff_object, None)?;

        Ok(id)
    }

    pub(super) fn map_alter_item(
        &mut self,
        key: &str,
        prev: impl IntoObjectMapContentItem,
        value: impl IntoObjectMapContentItem,
    ) {
        assert_eq!(self.content_type, ObjectMapSimpleContentType::Map);

        let prev = prev.into_content(None).into_set_item();
        let (key, value) = value.into_content(Some(key)).into_map_item();

        self.pending_results.push_back(ObjectMapPendingResult::Map((
            ObjectMapDiffAction::Alter,
            key,
            Some(prev),
            value,
        )));
    }

    pub(super) fn map_item(
        &mut self,
        action: ObjectMapDiffAction,
        key: &str,
        value: impl IntoObjectMapContentItem,
    ) {
        assert_eq!(self.content_type, ObjectMapSimpleContentType::Map);
        assert!(action != ObjectMapDiffAction::Alter);

        let (key, value) = value.into_content(Some(key)).into_map_item();

        self.pending_results
            .push_back(ObjectMapPendingResult::Map((action, key, None, value)));
    }

    pub(super) fn set_item(
        &mut self,
        action: ObjectMapDiffAction,
        value: impl IntoObjectMapContentItem,
    ) {
        assert_eq!(self.content_type, ObjectMapSimpleContentType::Set);

        let value = value.into_content(None).into_set_item();
        self.pending_results
            .push_back(ObjectMapPendingResult::Set((action, value)));
    }

    async fn deal_with_pending_result(&mut self) -> BuckyResult<()> {
        let cache = self.cache.clone();
        loop {
            let item = self.pending_results.pop_front();
            if item.is_none() {
                break;
            }

            let item = item.unwrap();
            match item {
                ObjectMapPendingResult::Map((action, key, prev, value)) => {
                    let item = match action {
                        ObjectMapDiffAction::Alter => {
                            assert!(prev.is_some());

                            // 如果需要展开，那么这里异步的计算
                            let diff = if self.expand_altered {
                                let prev = prev.as_ref().unwrap();

                                if prev.obj_type_code() == ObjectTypeCode::ObjectMap && value.obj_type_code() == ObjectTypeCode::ObjectMap {
                                    let cache = cache.clone();
                                    let prev = prev.to_owned();
                                    let value = value.clone();

                                    info!("will calc sub diff: prev={}, altered={}", prev, value);
                                    let diff_id = async_std::task::spawn(async move {
                                        Self::diff_objects(&cache, &prev, &value, true).await
                                    }).await?;

                                    Some(diff_id)
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            ObjectMapDiffMapItem {
                                prev,
                                altered: Some(value),
                                diff,
                            }
                        }
                        ObjectMapDiffAction::Add => {
                            assert!(prev.is_none());

                            ObjectMapDiffMapItem {
                                prev: None,
                                altered: Some(value),
                                diff: None,
                            }
                        }
                        ObjectMapDiffAction::Remove => {
                            assert!(prev.is_none());

                            ObjectMapDiffMapItem {
                                prev: Some(value),
                                altered: None,
                                diff: None,
                            }
                        }
                    };

                    self.result
                        .as_mut()
                        .unwrap()
                        .diff_insert_with_key(&cache, &key, &item)
                        .await?;
                }
                ObjectMapPendingResult::Set((action, value)) => {
                    let item = match action {
                        ObjectMapDiffAction::Add => ObjectMapDiffSetItem {
                            prev: None,
                            altered: Some(value),
                        },
                        ObjectMapDiffAction::Remove => ObjectMapDiffSetItem {
                            prev: Some(value),
                            altered: None,
                        },
                        _ => {
                            unreachable!();
                        }
                    };

                    self.result
                        .as_mut()
                        .unwrap()
                        .diff_insert(&cache, &item)
                        .await?;
                }
            }
        }

        Ok(())
    }

    // 添加一个pending的diff
    pub(crate) fn pend_async_sub_alter(&mut self, prev: &ObjectId, next: &ObjectId) {
        self.pending_diffs
            .push_back(ObjectMapPendingDiff::SubAlter((
                prev.to_owned(),
                next.to_owned(),
            )));
    }

    pub(crate) fn pend_async_alter(&mut self, prev: ObjectId, next: ObjectId) {
        self.pending_diffs.push_back(ObjectMapPendingDiff::Alter((
            prev.to_owned(),
            next.to_owned(),
        )));
    }

    pub(crate) fn pend_async_add(&mut self, value: &ObjectId) {
        self.pending_diffs
            .push_back(ObjectMapPendingDiff::Add(value.to_owned()));
    }

    pub(crate) fn pend_async_remove(&mut self, value: &ObjectId) {
        self.pending_diffs
            .push_back(ObjectMapPendingDiff::Remove(value.to_owned()));
    }

    async fn calc_diff(&mut self) -> BuckyResult<()> {
        loop {
            if self.pending_diffs.is_empty() {
                break;
            }

            let pending = self.pending_diffs.pop_front().unwrap();
            match pending {
                ObjectMapPendingDiff::SubAlter((prev, next)) => {
                    self.diff_recursive(&prev, &next).await?;
                }
                ObjectMapPendingDiff::Alter((prev, next)) => {
                    self.diff_hub_simple(&prev, &next).await?;
                }
                ObjectMapPendingDiff::Add(value) => {
                    self.diff_all(ObjectMapDiffAction::Add, &value).await?;
                }
                ObjectMapPendingDiff::Remove(value) => {
                    self.diff_all(ObjectMapDiffAction::Remove, &value).await?;
                }
            }
        }

        self.deal_with_pending_result().await?;

        Ok(())
    }

    // 将整个ObjectMap/SubObjectMap添加到目标列表，通过遍历算法获取所有元素
    async fn diff_all(
        &mut self,
        action: ObjectMapDiffAction,
        target: &ObjectId,
    ) -> BuckyResult<()> {
        assert!(action == ObjectMapDiffAction::Add || action == ObjectMapDiffAction::Remove);

        let target_obj = self.cache.get_object_map(target).await?;

        if target_obj.is_none() {
            let msg = format!("diff all but not found! target={}", target);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let target_obj = target_obj.unwrap();

        let mut it = ObjectMapBindIterator::new_with_target(target_obj, self.cache.clone()).await;

        while !it.is_end() {
            // 处理已经缓存的结果
            self.deal_with_pending_result().await?;

            let list = it.next(32).await?;
            for item in list.list {
                match item {
                    ObjectMapContentItem::Map((key, value)) => {
                        self.map_item(action.clone(), &key, value);
                    }
                    ObjectMapContentItem::Set(value) => {
                        self.set_item(action.clone(), value);
                    }
                    _ => unreachable!(),
                }
            }
        }

        Ok(())
    }

    async fn diff_recursive(&mut self, prev: &ObjectId, next: &ObjectId) -> BuckyResult<()> {
        let prev = self.cache.get_object_map(prev).await?;
        let next = self.cache.get_object_map(next).await?;

        if prev.is_none() || next.is_none() {
            let msg = format!(
                "recursive diff object but not found! prev={:?}, next={:?}",
                prev, next
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let prev = prev.unwrap();
        let next = next.unwrap();
        let prev_obj = prev.lock().await;
        let next_obj = next.lock().await;
        prev_obj.diff(&next_obj, self);

        Ok(())
    }

    // hub和simple模式的两个ObjectMap求diff，通过遍历的方法
    async fn diff_hub_simple(&mut self, prev: &ObjectId, next: &ObjectId) -> BuckyResult<()> {
        let prev = self.cache.get_object_map(prev).await?;
        let next = self.cache.get_object_map(next).await?;

        if prev.is_none() || next.is_none() {
            let msg = format!(
                "diff object but not found! prev={:?}, next={:?}",
                prev, next
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let prev = prev.unwrap();
        let next = next.unwrap();

        // 遍历查找修改和移除的元素
        let mut it = ObjectMapBindIterator::new_with_target(prev.clone(), self.cache.clone()).await;
        while !it.is_end() {
            // 处理已经缓存的结果
            self.deal_with_pending_result().await?;

            let list = it.next(32).await?;
            for item in list.list {
                let next_obj = next.lock().await;

                match item {
                    ObjectMapContentItem::Map((key, value)) => {
                        if let Some(next_value) = next_obj.get_by_key(&self.cache, &key).await? {
                            if value != next_value {
                                self.map_alter_item(&key, value, next_value);
                            }
                        } else {
                            self.map_item(ObjectMapDiffAction::Remove, &key, value);
                        }
                    }
                    ObjectMapContentItem::Set(value) => {
                        if !next_obj.contains(&self.cache, &value).await? {
                            self.set_item(ObjectMapDiffAction::Remove, value);
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }

        let mut it = ObjectMapBindIterator::new_with_target(next, self.cache.clone()).await;
        while !it.is_end() {
            // 处理已经缓存的结果
            self.deal_with_pending_result().await?;

            let list = it.next(32).await?;
            for item in list.list {
                let prev_obj = prev.lock().await;

                match item {
                    ObjectMapContentItem::Map((key, value)) => {
                        if let None = prev_obj.get_by_key(&self.cache, &key).await? {
                            self.map_item(ObjectMapDiffAction::Add, &key, value);
                        }
                    }
                    ObjectMapContentItem::Set(value) => {
                        if !prev_obj.contains(&self.cache, &value).await? {
                            self.set_item(ObjectMapDiffAction::Add, value);
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }

        Ok(())
    }

    // dest - source = diff => source + diff = dest
    pub async fn apply_diff(
        cache: &ObjectMapOpEnvCacheRef,
        source_id: &ObjectId,
        diff_id: &ObjectId,
    ) -> BuckyResult<ObjectId> {
        assert_ne!(source_id, diff_id);

        let source = cache.get_object_map(source_id).await?;
        if source.is_none() {
            let msg = format!("source object not found! target={}", source_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        let source = source.unwrap();

        let diff = cache.get_object_map(diff_id).await?;
        if diff.is_none() {
            let msg = format!("diff object not found! target={}", diff_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        let diff = diff.unwrap();

        let source = source.lock().await;

        // 检查类型是否匹配
        let content_type = source.content_type();
        let diff_content_type = diff.lock().await.content_type();
        if !content_type.is_diff_match(&diff_content_type) {
            let msg = format!(
                "apply diff with unmatched objectmap content type: source={:?}, diff={:?}",
                content_type, diff_content_type
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        // source需要clone一份再修改
        let mut source = source.clone();

        let mut it = ObjectMapBindIterator::new_with_target(diff, cache.clone()).await;
        while !it.is_end() {
            let list = it.next(32).await?;
            for item in list.list {
                match item {
                    ObjectMapContentItem::DiffMap((key, value)) => {
                       if value.prev.is_none() {
                            // add action
                            // FIXME 如果出现了错误项，是否继续？
                            if value.altered.is_none() {
                                error!("invalid diffmap content item: {}", value);
                                continue;
                            }
                            debug!("will apply diff added item: {}={}", key, value);

                            let altered = value.altered.unwrap();
                            // 这里使用set_with_key而不是insert_with_key，是为了处理已经存在情况下的容错
                            let ret = source
                                .set_with_key(&cache, &key, &altered, &None, true)
                                .await?;
                            if ret.is_some() {
                                error!("apply added diff but key/value already exists! key={}, added={}, current={:?}", key, altered, ret);
                            }
                        } else if value.altered.is_none() {
                            // remove action
                            debug!("will apply diff removed item: {}={}", key, value);

                            // FIXME 这里是否需要对当前状态进行检查？
                            let prev = value.prev.unwrap();
                            let ret = source.remove_with_key(&cache, &key, &None).await?;
                            if ret != Some(prev) {
                                error!("apply removed diff but key/value not match! key={}, removed={}, current={:?}", key, prev, ret);
                            }
                        } else {
                            // altered action
                            let prev = value.prev.as_ref().unwrap();
                            let altered = value.altered.as_ref().unwrap();
                            debug!("will apply diff altered item: {}={}", key, value);

                            // FIXME 这里是否需要对当前状态进行检查？
                            let ret = source
                                .set_with_key(&cache, &key, &altered, &None, false)
                                .await?;
                            if ret != Some(*prev) {
                                error!(
                                    "apply altered diff but prev value not the same! key={}, prev={}, altered={}, current={:?}",
                                    key, prev, altered, ret,
                                );
                            }

                            if value.diff.is_some() {
                                if value.diff.as_ref().unwrap().obj_type_code() == ObjectTypeCode::ObjectMap {
                                    debug!("will apply diff recursive: {}={}", key, value);
                                    Self::apply_diff_recursive(cache, value).await?;
                                } else {
                                    // allow some user defined diff object for leaf node
                                    debug!("ignore diff object type: {}={}", key, value);
                                }
                            }
                        }
                    }
                    ObjectMapContentItem::DiffSet(value) => {
                        if value.prev.is_some() {
                            debug!("will apply removed diff: {}", value);

                            let prev = value.prev.unwrap();
                            let ret = source.remove(&cache, &prev).await?;
                            if !ret {
                                error!("apply removed diff but not found! value={}", prev);
                            }
                        } else if value.altered.is_some() {
                            debug!("will apply added diff: {}", value);

                            let altered = value.altered.unwrap();
                            let ret = source.insert(&cache, &altered).await?;
                            if !ret {
                                error!("apply added diff but already exists! value={}", altered);
                            }
                        } else {
                            // FIXME 如果出现了错误项，是否继续？
                            error!("invalid diffmap content item: {}", value);
                            continue;
                        }
                    }
                    _ => {
                        unreachable!();
                    }
                }
            }
        }

        // 刷新id
        let id = source.flush_id();

        // 如果和源对象一致
        if id == *source_id {
            warn!(
                "apply diff to source object but not changed! source={}, diff={}",
                source_id, diff_id
            );
            return Ok(id);
        }

        info!(
            "apply diff to source object success! source={}, diff={}, result={}",
            source_id, diff_id, id
        );
        cache.put_object_map(&id, source, None)?;

        Ok(id)
    }

    #[async_recursion::async_recursion]
    async fn apply_diff_recursive(
        cache: &ObjectMapOpEnvCacheRef,
        diff_item: ObjectMapDiffMapItem,
    ) -> BuckyResult<ObjectId> {
        let prev_id = diff_item.prev.unwrap();
        let diff_id = diff_item.diff.unwrap();

        let prev_code = prev_id.obj_type_code();
        let diff_code = diff_id.obj_type_code();

        if prev_code != ObjectTypeCode::ObjectMap || diff_code != ObjectTypeCode::ObjectMap {
            let msg = format!("apply diff but recursive but invalid objectmap object type! prev={}, diff={}, prev_code={:?}, diff_code={:?}",
                prev_id, diff_id, prev_code, diff_code);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        let cache = cache.clone();
        async_std::task::spawn(async move {
            let new_id = Self::apply_diff(&cache, &prev_id, &diff_id).await?;

            /*
            // for debug
            let root = cache.get_object_map(&diff_item.altered.as_ref().unwrap()).await.unwrap();
            let root = root.unwrap();
            {
                let obj = root.lock().await;
                let id = obj.flush_id_without_cache();
                assert_eq!(id, *diff_item.altered.as_ref().unwrap());
            }
            let mut it = ObjectMapPathIterator::new(root, cache.clone()).await;
            while !it.is_end() {
                let list = it.next(1).await.unwrap();
                info!("altered list: {} {:?}", 1, list.list);
            }


            let root = cache.get_object_map(&new_id).await.unwrap();
            let root = root.unwrap();
            {
                let obj = root.lock().await;
                let id = obj.flush_id_without_cache();
                assert_eq!(id, new_id);
            }
            let mut it = ObjectMapPathIterator::new(root, cache.clone()).await;
            while !it.is_end() {
                let list = it.next(1).await.unwrap();
                info!("result list: {} {:?}", 1, list.list);
            }
            */

            // 校验结果是否一致
            if Some(new_id) != diff_item.altered {
                let msg = format!("apply diff but got unmatch result: prev={}, diff={}, expect={:?}, got={}",
                    prev_id, diff_id, diff_item.altered, new_id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }

            Ok(new_id)
        })
        .await
    }

    // 对diff的alter内容进行展开，返回一个新的对象(如果有实际的展开内容)
    async fn expand_altered(
        cache: &ObjectMapOpEnvCacheRef,
        diff_id: &ObjectId,
    ) -> BuckyResult<ObjectId> {
        let diff = cache.get_object_map(diff_id).await?;
        if diff.is_none() {
            let msg = format!("diff object not found! target={}", diff_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        let diff = diff.unwrap();

        let mut it = ObjectMapBindIterator::new_with_target(diff.clone(), cache.clone()).await;

        // 用以保存展开后的alered对象
        let mut expanded_altered = None;

        while !it.is_end() {
            let list = it.next(32).await?;
            for item in list.list {
                match item {
                    ObjectMapContentItem::DiffMap((key, item)) => {
                        if item.prev.is_none() || item.altered.is_none() {
                            continue;
                        }
                        assert!(item.diff.is_none());

                        let prev = item.prev.as_ref().unwrap();
                        let altered = item.altered.as_ref().unwrap();

                        info!(
                            "will expand child diff: key={}, prev={}, altered={}",
                            key, prev, altered
                        );

                        // prev和altered必须都是ObjectMap，才可以继续递归的diff
                        let diff_id = Self::diff_and_expand_recursive(cache, prev, altered).await?;
                        if diff_id != *altered {
                            // 增加diff字段，替换现有的项
                            if expanded_altered.is_none() {
                                // 把diff clone一份
                                expanded_altered = Some(diff.lock().await.clone());
                            }

                            let mut expanded_item = item.clone();
                            expanded_item.diff = Some(diff_id);

                            expanded_altered
                                .as_mut()
                                .unwrap()
                                .diff_set_with_key(cache, &key, &expanded_item, &Some(item), false)
                                .await?;
                        }
                    }
                    ObjectMapContentItem::DiffSet(_) => {
                        // set类型不需要展开
                    }
                    _ => {
                        error!("expand altered but with unmatch objectmap content type: diff={}, content_type={:?}", diff_id, item.content_type());
                    }
                }
            }
        }

        if let Some(expanded_altered) = expanded_altered {
            let new_diff_id = expanded_altered.flush_id();
            assert_ne!(new_diff_id, *diff_id);
            cache.put_object_map(&new_diff_id, expanded_altered, None)?;

            Ok(new_diff_id)
        } else {
            Ok(diff_id.clone())
        }
    }

    #[async_recursion::async_recursion]
    async fn diff_and_expand_recursive(
        cache: &ObjectMapOpEnvCacheRef,
        prev_id: &ObjectId,
        altered_id: &ObjectId,
    ) -> BuckyResult<ObjectId> {
        let prev_code = prev_id.obj_type_code();
        let altered_code = altered_id.obj_type_code();

        if prev_code != ObjectTypeCode::ObjectMap || altered_code != ObjectTypeCode::ObjectMap {
            return Ok(altered_id.to_owned());
        }

        let cache = cache.clone();
        let prev_id = prev_id.to_owned();
        let altered_id = altered_id.to_owned();
        async_std::task::spawn(async move {
            let diff_id = Self::diff_objects(&cache, &prev_id, &altered_id, false).await?;
            Self::expand_altered(&cache, &diff_id).await
        })
        .await
    }

    // 打印一个diff的内容
    pub async fn dump_diff(cache: &ObjectMapOpEnvCacheRef, diff_id: &ObjectId) -> BuckyResult<()> {
        let diff = cache.get_object_map(diff_id).await?;

        if diff.is_none() {
            let msg = format!("diff object not found! target={}", diff_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let diff = diff.unwrap();

        info!("will dump diff objectmap: {}", diff_id);

        let mut it = ObjectMapBindIterator::new_with_target(diff, cache.clone()).await;

        let mut index = 0;
        while !it.is_end() {
            let list = it.next(8).await?;
            for item in list.list {
                match item {
                    ObjectMapContentItem::Map((key, value)) => {
                        info!("[{}] map item: {}={}", index, key, value);
                        index += 1;
                    }
                    ObjectMapContentItem::DiffMap((key, value)) => {

                        info!("[{}] map diff item: {:?}, {}={}", index, value.action(), key, value);
                        index += 1;
                    }
                    ObjectMapContentItem::Set(value) => {
                        info!("[{}] set item: {}", index, value);
                        index += 1;
                    }
                    ObjectMapContentItem::DiffSet(value) => {
                        info!("[{}] set diff item: {:?}, {}", index, value.action(), value);
                        index += 1;
                    }
                }
            }
        }

        info!("end dump diff objectmap: {}", diff_id);

        Ok(())
    }
}


#[cfg(test)]
mod test {
    use super::super::cache::*;
    use super::super::object_map::*;
    use super::*;

    use std::str::FromStr;

    async fn gen_prev(cache: &ObjectMapOpEnvCacheRef) -> ObjectId {
        let owner = ObjectId::default();
        let mut map = ObjectMap::new(
            ObjectMapSimpleContentType::Map,
            Some(owner.clone()),
            Some(owner.clone()),
        )
        .no_create_time()
        .build();

        for i in 0..1000 {
            let key = format!("test_map_{:0>3}", i);
            let object_id = ObjectId::default();
            info!("begin insert_with_key: {}", key);
            map.insert_with_key(&cache, &key, &object_id).await.unwrap();
            info!("end insert_with_key: {}", key);
        }

        let id = map.flush_id();
        cache.put_object_map(&id, map, None).unwrap();

        id
    }

    async fn gen_next(cache: &ObjectMapOpEnvCacheRef) -> ObjectId {
        let owner = ObjectId::default();
        let mut map = ObjectMap::new(
            ObjectMapSimpleContentType::Map,
            Some(owner.clone()),
            Some(owner.clone()),
        )
        .no_create_time()
        .build();

        for i in 500..1500 {
            let key = format!("test_map_{:0>3}", i);

            //let object_id = ObjectId::default();
            let object_id =
                ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();

            info!("begin insert_with_key: {}", key);
            map.insert_with_key(&cache, &key, &object_id).await.unwrap();
            info!("end insert_with_key: {}", key);
        }

        let id = map.flush_id();
        cache.put_object_map(&id, map, None).unwrap();

        id
    }

    async fn gen_next2(cache: &ObjectMapOpEnvCacheRef) -> ObjectId {
        let owner = ObjectId::default();
        let mut map = ObjectMap::new(
            ObjectMapSimpleContentType::Map,
            Some(owner.clone()),
            Some(owner.clone()),
        )
        .no_create_time()
        .build();

        for i in 0..1 {
            let key = format!("test_map_{:0>3}", i);

            //let object_id = ObjectId::default();
            let object_id =
                ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();

            info!("begin insert_with_key: {}", key);
            map.insert_with_key(&cache, &key, &object_id).await.unwrap();
            info!("end insert_with_key: {}", key);
        }

        let id = map.flush_id();
        cache.put_object_map(&id, map, None).unwrap();

        id
    }

    async fn test_diff() {
        let noc = ObjectMapMemoryNOCCache::new();
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(None, noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        let prev_id = gen_prev(&cache).await;
        let next_id = gen_next(&cache).await;

        let diff_id = ObjectMapDiff::diff_objects(&cache, &prev_id, &next_id, false)
            .await
            .unwrap();
        ObjectMapDiff::dump_diff(&cache, &diff_id).await.unwrap();

        let new_next_id = ObjectMapDiff::apply_diff(&cache, &prev_id, &diff_id)
            .await
            .unwrap();

        /*
        let new_diff_id = ObjectMapDiff::diff_objects(&cache, &next_id, &new_next_id)
        .await
        .unwrap();
        ObjectMapDiff::dump_diff(&cache, &new_diff_id).await.unwrap();
        */
        assert_eq!(new_next_id, next_id);
    }

    #[test]
    fn test() {
        crate::init_simple_log("test-object-map-diff", Some("debug"));
        async_std::task::block_on(async move {
            //test_set().await;
            //test_map().await;
            test_diff().await;
        });
    }
}

#[cfg(test)]
mod test_path_diff {
    use super::super::cache::*;
    use super::super::object_map::*;
    use super::super::path::*;
    use super::super::path_iterator::*;
    use super::*;

    use std::str::FromStr;

    async fn gen_path1(cache: &ObjectMapOpEnvCacheRef, root_id: &ObjectId) -> ObjectId {
        // 这个用以测试objectmap由于不存在无法继续展开的情况
        // let x_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
        let x_value = ObjectId::from_str("5aSixgPg3hDa1oU9eAtRcKTyVKg5X2bVXWPVhk3U5c7G").unwrap();

        let path = ObjectMapPath::new(root_id.clone(), cache.clone(), false);
        path.insert_with_path("/a/b/c", &x_value).await.unwrap();
        path.insert_with_path("/a/b/d", &x_value).await.unwrap();
        //path.insert_with_path("/a/x", &x_value).await.unwrap();
        path.insert("/a/z", &x_value).await.unwrap();

        path.root()
    }

    async fn gen_path2(cache: &ObjectMapOpEnvCacheRef, root_id: &ObjectId) -> ObjectId {
        // let x_value = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();
        let x_value = ObjectId::from_str("5aSixgPCivmQfASRbjAvBiwgxhU8LrNtYtC2D6Lis2NQ").unwrap();

        let path = ObjectMapPath::new(root_id.clone(), cache.clone(), false);
        path.insert_with_path("/a/b/c", &x_value).await.unwrap();
        path.insert_with_path("/a/b/d", &x_value).await.unwrap();
        path.insert_with_path("/a/x", &x_value).await.unwrap();
        path.insert("/a/z", &x_value).await.unwrap();
        path.insert("/a/z1", &x_value).await.unwrap();
        path.insert("/a/z2", &x_value).await.unwrap();

        path.root()
    }

    async fn test1(cache: &ObjectMapOpEnvCacheRef) {

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

        let path1 = gen_path1(cache, &root_id).await;
        let path2 = gen_path2(cache, &root_id).await;

        let diff_id = ObjectMapDiff::diff_objects(&cache, &path1, &path2, true).await.unwrap();
        ObjectMapDiff::dump_diff(&cache, &diff_id).await.unwrap();

        // 测试diff遍历
        let root = cache.get_object_map(&diff_id).await.unwrap();
        let mut it = ObjectMapPathIterator::new(root.unwrap(), cache.clone(), ObjectMapPathIteratorOption::default()).await;
        while !it.is_end() {
            let list = it.next(1).await.unwrap();
            info!("list: {} {:?}", 1, list.list);
        }

        // 测试apply
        let new_2 = ObjectMapDiff::apply_diff(&cache, &path1, &diff_id).await.unwrap();
        let root = cache.get_object_map(&new_2).await.unwrap();
        let mut it = ObjectMapPathIterator::new(root.unwrap(), cache.clone(), ObjectMapPathIteratorOption::default()).await;
        while !it.is_end() {
            let list = it.next(1).await.unwrap();
            info!("list: {} {:?}", 1, list.list);
        }

        assert_eq!(new_2, path2);
    }

    async fn test_diff() {
        let noc = ObjectMapMemoryNOCCache::new();
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(None, noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        test1(&cache).await;
    }

    #[test]
    fn test() {
        crate::init_simple_log("test-object-map-path-diff", Some("debug"));
        async_std::task::block_on(async move {
            test_diff().await;
        });
    }
}
