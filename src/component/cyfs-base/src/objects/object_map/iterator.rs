use super::cache::*;
use super::diff::*;
use super::object_map::*;
use crate::*;

use serde::Serialize;

#[derive(Clone, Debug, RawEncode, RawDecode, Eq, PartialEq, Serialize)]
pub struct ObjectMapDiffMapItem {
    pub prev: Option<ObjectId>,
    pub altered: Option<ObjectId>,
    pub diff: Option<ObjectId>,
}

impl ObjectMapDiffMapItem {
    pub fn action(&self) -> ObjectMapDiffAction {
        if self.prev.is_none() {
            assert!(self.altered.is_some());
            ObjectMapDiffAction::Add
        } else if self.altered.is_none() {
            assert!(self.prev.is_some());
            ObjectMapDiffAction::Remove
        } else {
            ObjectMapDiffAction::Alter
        }
    }
}

impl std::fmt::Display for ObjectMapDiffMapItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "prev={:?}, altered={:?}, diff={:?}",
            self.prev, self.altered, self.diff
        )
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct ObjectMapDiffSetItem {
    pub prev: Option<ObjectId>,
    pub altered: Option<ObjectId>,
}

impl ObjectMapDiffSetItem {
    pub fn as_slice(&self) -> &[u8] {
        match &self.prev {
            Some(value) => value.as_slice(),
            None => self.altered.as_ref().unwrap().as_slice(),
        }
    }

    pub fn action(&self) -> ObjectMapDiffAction {
        if self.prev.is_none() {
            assert!(self.altered.is_some());
            ObjectMapDiffAction::Add
        } else if self.altered.is_none() {
            assert!(self.prev.is_some());
            ObjectMapDiffAction::Remove
        } else {
            unreachable!();
        }
    }
}

impl std::fmt::Display for ObjectMapDiffSetItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(id) = self.prev {
            write!(f, "-{}", id)
        } else if let Some(id) = self.altered {
            write!(f, "+{}", id)
        } else {
            write!(f, "None")
        }
        // write!(f, "prev={:?}, altered={:?}", self.prev, self.altered)
    }
}

pub trait IntoObjectMapContentItem {
    fn into_content(self, key: Option<&str>) -> ObjectMapContentItem;
}

impl IntoObjectMapContentItem for ObjectMapDiffMapItem {
    fn into_content(self, key: Option<&str>) -> ObjectMapContentItem {
        ObjectMapContentItem::DiffMap((key.unwrap().to_owned(), self))
    }
}
impl IntoObjectMapContentItem for ObjectMapDiffSetItem {
    fn into_content(self, key: Option<&str>) -> ObjectMapContentItem {
        assert!(key.is_none());
        ObjectMapContentItem::DiffSet(self)
    }
}
impl IntoObjectMapContentItem for ObjectId {
    fn into_content(self, key: Option<&str>) -> ObjectMapContentItem {
        match key {
            Some(key) => ObjectMapContentItem::Map((key.to_owned(), self)),
            None => ObjectMapContentItem::Set(self),
        }
    }
}

#[derive(Debug, Eq, PartialEq,)]
pub enum ObjectMapContentItem {
    DiffMap((String, ObjectMapDiffMapItem)),
    Map((String, ObjectId)),
    DiffSet(ObjectMapDiffSetItem),
    Set(ObjectId),
}

impl ObjectMapContentItem {
    pub fn content_type(&self) -> ObjectMapSimpleContentType {
        match &self {
            Self::Map(_) => ObjectMapSimpleContentType::Map,
            Self::DiffMap(_) => ObjectMapSimpleContentType::DiffMap,
            Self::Set(_) => ObjectMapSimpleContentType::Set,
            Self::DiffSet(_) => ObjectMapSimpleContentType::DiffSet,
        }
    }

    pub fn into_map_item(self) -> (String, ObjectId) {
        match self {
            Self::Map(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn into_diff_map_item(self) -> (String, ObjectMapDiffMapItem) {
        match self {
            Self::DiffMap(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn into_set_item(self) -> ObjectId {
        match self {
            Self::Set(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn into_diff_set_item(self) -> ObjectMapDiffSetItem {
        match self {
            Self::DiffSet(value) => value,
            _ => unreachable!(),
        }
    }
}

impl std::fmt::Display for ObjectMapContentItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ObjectMapContentItem::Map((key, value)) => {
                write!(f, "({}: {}),", key, value)?;
            }
            ObjectMapContentItem::DiffMap((key, value)) => {
                match value.action() {
                    ObjectMapDiffAction::Add => match &value.altered {
                        Some(id) => write!(f, "(+ {}: {}),", key, id)?,
                        None => write!(f, "(+ {}: None),", key)?,
                    },
                    ObjectMapDiffAction::Remove => match &value.prev {
                        Some(id) => write!(f, "(- {}: {}),", key, id)?,
                        None => write!(f, "(- {}: None),", key)?,
                    },
                    ObjectMapDiffAction::Alter => {
                        // action返回Alter情况下，prev和altered一定不为空
                        write!(
                            f,
                            "({}: {} -> {}),",
                            key,
                            value.prev.as_ref().unwrap(),
                            value.altered.as_ref().unwrap()
                        )?;
                    }
                }
            }
            ObjectMapContentItem::Set(value) => {
                write!(f, "({}),", value)?;
            }
            ObjectMapContentItem::DiffSet(value) => match value.action() {
                ObjectMapDiffAction::Add => match &value.altered {
                    Some(id) => {
                        write!(f, "(+ {}),", id)?;
                    }
                    None => write!(f, "(+ None),")?,
                },
                ObjectMapDiffAction::Remove => match &value.prev {
                    Some(id) => {
                        write!(f, "(- {}),", id)?;
                    }
                    None => write!(f, "(- None),")?,
                },
                _ => {
                    unreachable!();
                }
            },
        }

        Ok(())
    }
}

pub struct ObjectMapContentList {
    pub list: Vec<ObjectMapContentItem>,
}

impl std::fmt::Display for ObjectMapContentList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in &self.list {
            write!(f, "{},", item)?;
        }

        Ok(())
    }
}

impl ObjectMapContentList {
    pub fn new(capacity: usize) -> Self {
        Self {
            list: Vec::with_capacity(capacity),
        }
    }

    pub fn push_key_value(&mut self, key: &str, value: impl IntoObjectMapContentItem) {
        let item = value.into_content(Some(key));
        self.list.push(item);
    }

    pub fn push_value(&mut self, value: impl IntoObjectMapContentItem) {
        let item = value.into_content(None);
        self.list.push(item);
    }

    pub fn take(&mut self) -> ObjectMapContentList {
        let mut ret = ObjectMapContentList::new(0);
        std::mem::swap(&mut ret.list, &mut self.list);

        ret
    }
}

#[derive(Debug, Clone)]
pub enum SetIteratorPostion {
    DiffSet(ObjectMapDiffSetItem),
    Set(ObjectId),
}

impl From<ObjectId> for SetIteratorPostion {
    fn from(value: ObjectId) -> Self {
        Self::Set(value)
    }
}

impl From<ObjectMapDiffSetItem> for SetIteratorPostion {
    fn from(value: ObjectMapDiffSetItem) -> Self {
        Self::DiffSet(value)
    }
}

/*
impl Into<SetIteratorPostion> for ObjectId {
    fn into(self) -> SetIteratorPostion {
        SetIteratorPostion::Set(self)
    }
}

impl Into<SetIteratorPostion> for ObjectMapDiffSetItem {
    fn into(self) -> SetIteratorPostion {
        SetIteratorPostion::DiffSet(self)
    }
}
*/

impl From<SetIteratorPostion> for ObjectId {
    fn from(value: SetIteratorPostion) -> Self {
        match value {
            SetIteratorPostion::Set(v) => v,
            _ => unreachable!(),
        }
    }
}

impl From<SetIteratorPostion> for ObjectMapDiffSetItem {
    fn from(value: SetIteratorPostion) -> Self {
        match value {
            SetIteratorPostion::DiffSet(v) => v,
            _ => unreachable!(),
        }
    }
}

/*
impl Into<ObjectId> for SetIteratorPostion {
    fn into(self) -> ObjectId {
        match self {
            Self::Set(v) => v,
            _ => unreachable!(),
        }
    }
}

impl Into<ObjectMapDiffSetItem> for SetIteratorPostion {
    fn into(self) -> ObjectMapDiffSetItem {
        match self {
            Self::DiffSet(v) => v,
            _ => unreachable!(),
        }
    }
}
*/

#[derive(Debug, Clone)]
pub enum IteratorPosition {
    Hub(Option<u16>),
    SimpleSet(Option<SetIteratorPostion>),
    SimpleMap(Option<String>),
}

impl IteratorPosition {
    pub fn into_hub(self) -> Option<u16> {
        match self {
            Self::Hub(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn into_simple_map(self) -> Option<String> {
        match self {
            Self::SimpleMap(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn into_simple_set(self) -> Option<SetIteratorPostion> {
        match self {
            Self::SimpleSet(value) => value,
            _ => unreachable!(),
        }
    }
}
struct HubContentIterator {
    object_id: ObjectId,
    pos: Option<String>,
}

enum ObjectMapIteratorResult {
    Iterator(ObjectMapContentList),
    Skip(usize),
}

impl ObjectMapIteratorResult {
    pub fn len(&self) -> usize {
        match self {
            Self::Iterator(r) => r.list.len(),
            Self::Skip(count) => *count,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push_key_value(&mut self, key: &str, value: impl IntoObjectMapContentItem) {
        match self {
            Self::Iterator(r) => r.push_key_value(key, value),
            Self::Skip(count) => *count += 1,
        }
    }

    pub fn push_value(&mut self, value: impl IntoObjectMapContentItem) {
        match self {
            Self::Iterator(r) => r.push_value(value),
            Self::Skip(count) => *count += 1,
        }
    }

    pub fn take_iterator_result(&mut self) -> ObjectMapContentList {
        match self {
            Self::Iterator(r) => r.take(),
            Self::Skip(_) => unreachable!(),
        }
    }

    pub fn take_skip_result(&mut self) -> usize {
        match self {
            Self::Skip(count) => {
                let ret = *count;
                *count = 0;
                ret
            }
            Self::Iterator(_) => unreachable!(),
        }
    }
}

pub struct ObjectMapIterator {
    pub cache: ObjectMapOpEnvCacheRef,

    // 每次步进的元素个数
    step: usize,
    // 读取到的元素个数
    result: ObjectMapIteratorResult,

    // 保存深度遍历状态的栈
    depth: usize,
    stack: Vec<IteratorPosition>,

    // 是否结束
    is_end: bool,
}

impl ObjectMapIterator {
    pub fn new(skip: bool, target: &ObjectMap, cache: ObjectMapOpEnvCacheRef) -> Self {
        let result = if skip {
            ObjectMapIteratorResult::Skip(0)
        } else {
            ObjectMapIteratorResult::Iterator(ObjectMapContentList::new(1))
        };

        let mut it = Self {
            cache,
            step: 1,
            result,
            depth: 0,
            stack: vec![],
            is_end: false,
        };

        it.push_empty_pos(target.mode(), target.content_type());
        it
    }

    // skip -> intertor
    pub fn into_iterator(mut self) -> Self {
        match self.result {
            ObjectMapIteratorResult::Skip(_) => {
                self.result = ObjectMapIteratorResult::Iterator(ObjectMapContentList::new(1));
            }
            ObjectMapIteratorResult::Iterator(_) => unreachable!(),
        }

        self
    }

    pub(crate) fn mark_end(&mut self) {
        assert!(!self.is_end);
        self.is_end = true;
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }

    pub fn step(&self) -> usize {
        self.step
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn reset_depth(&mut self) {
        self.depth = 0;
    }

    pub fn current_pos(&self) -> IteratorPosition {
        assert!(self.stack.len() > self.depth);
        self.stack[self.depth].clone()
    }

    pub fn update_pos(&mut self, depth: usize, pos: IteratorPosition) {
        assert!(self.stack.len() > depth);
        self.stack[depth] = pos;
    }

    pub fn is_enough(&self) -> bool {
        let total = self.result.len();
        if total < self.step {
            false
        } else {
            assert_eq!(total, self.step);
            true
        }
    }

    pub fn inc_depth(
        &mut self,
        mode: ObjectMapContentMode,
        content_type: ObjectMapSimpleContentType,
    ) {
        self.depth += 1;
        if self.depth == self.stack.len() {
            self.push_empty_pos(mode, content_type);
        }
    }

    fn push_empty_pos(
        &mut self,
        mode: ObjectMapContentMode,
        content_type: ObjectMapSimpleContentType,
    ) {
        let new_pos = match mode {
            ObjectMapContentMode::Simple => match content_type {
                ObjectMapSimpleContentType::Map | ObjectMapSimpleContentType::DiffMap => {
                    IteratorPosition::SimpleMap(None)
                }
                ObjectMapSimpleContentType::Set | ObjectMapSimpleContentType::DiffSet => {
                    IteratorPosition::SimpleSet(None)
                }
            },
            ObjectMapContentMode::Hub => IteratorPosition::Hub(None),
        };

        self.stack.push(new_pos);
    }

    pub fn dec_depth(&mut self) {
        assert!(self.depth > 0);
        assert_eq!(self.depth, self.stack.len() - 1);

        self.depth -= 1;
        self.stack.pop().unwrap();
    }

    pub fn push_key_value(&mut self, key: &str, value: impl IntoObjectMapContentItem) {
        self.result.push_key_value(key, value);
    }

    pub fn push_value(&mut self, value: impl IntoObjectMapContentItem) {
        self.result.push_value(value);
    }

    pub async fn next(
        &mut self,
        target: &ObjectMap,
        step: usize,
    ) -> BuckyResult<ObjectMapContentList> {
        assert!(self.result.is_empty());

        if self.is_end() {
            return Ok(ObjectMapContentList::new(0));
        }

        if step == 0 {
            return Ok(ObjectMapContentList::new(0));
        }

        // 每次迭代可以指定不同的step
        self.step = step;

        // 重置当前的遍历深度
        self.reset_depth();

        {
            target.next(self).await?;
        }

        let result = self.result.take_iterator_result().take();
        if result.list.len() < self.step {
            self.mark_end();
        }

        Ok(result)
    }

    pub async fn skip(&mut self, target: &ObjectMap, step: usize) -> BuckyResult<usize> {
        assert!(self.result.is_empty());

        if self.is_end() {
            return Ok(0);
        }

        if step == 0 {
            return Ok(0);
        }

        // 每次迭代可以指定不同的step
        self.step = step;

        // 重置当前的遍历深度
        self.reset_depth();

        {
            target.next(self).await?;
        }

        let count = self.result.take_skip_result();
        if count < self.step {
            self.mark_end();
        }

        Ok(count)
    }
}

pub struct ObjectMapBindIterator {
    target: ObjectMapRef,
    iterator: ObjectMapIterator,
}

impl ObjectMapBindIterator {
    pub async fn new_with_target(target: ObjectMapRef, cache: ObjectMapOpEnvCacheRef) -> Self {
        let iterator = {
            let obj = target.lock().await;
            ObjectMapIterator::new(false, &obj, cache)
        };

        Self { target, iterator }
    }

    pub async fn next(&mut self, step: usize) -> BuckyResult<ObjectMapContentList> {
        let target = self.target.clone();
        let obj = target.lock().await;
        self.iterator.next(&obj, step).await
    }
}

impl std::ops::Deref for ObjectMapBindIterator {
    type Target = ObjectMapIterator;
    fn deref(&self) -> &ObjectMapIterator {
        &self.iterator
    }
}

#[cfg(test)]
mod test {
    use super::super::cache::*;
    use super::*;

    use async_std::sync::Mutex as AsyncMutex;
    use std::collections::HashSet;
    use std::sync::Arc;

    async fn test_iterator() {
        let noc = ObjectMapMemoryNOCCache::new();
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(None, noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        let owner = ObjectId::default();
        let mut map = ObjectMap::new(
            ObjectMapSimpleContentType::Map,
            Some(owner.clone()),
            Some(owner.clone()),
        )
        .no_create_time()
        .build();

        let mut origin_keys = HashSet::new();
        for i in 0..1000 {
            let key = format!("test_map_{:0>3}", i);
            let object_id = ObjectId::default();
            info!("begin insert_with_key: {}", key);
            map.insert_with_key(&cache, &key, &object_id).await.unwrap();
            info!("end insert_with_key: {}", key);

            let ret = origin_keys.insert(key);
            assert!(ret);
        }

        let obj = Arc::new(AsyncMutex::new(map));
        let mut it = ObjectMapBindIterator::new_with_target(obj, cache).await;
        let mut step = 0;
        let mut got_keys = HashSet::new();
        while !it.is_end() {
            let list = it.next(step + 1).await.unwrap();
            info!("list: {} {:?}", step, list.list);
            step += 1;
            for item in list.list {
                let ret = got_keys.insert(item.into_map_item().0);
                assert!(ret);
            }
        }

        assert_eq!(origin_keys, got_keys);
        info!("iterate complete, count={}", got_keys.len());
    }

    #[test]
    fn test() {
        crate::init_simple_log("test-object-map-iterator", Some("debug"));
        async_std::task::block_on(async move {
            test_iterator().await;
        });
    }
}
