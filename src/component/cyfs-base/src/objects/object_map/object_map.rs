use super::cache::*;
use super::diff::*;
use super::iterator::*;
use super::visitor::*;
use crate::codec as cyfs_base;
use crate::*;
use crate::{RawDecode, RawEncode, RawEncodePurpose};

use async_std::sync::Mutex as AsyncMutex;
use sha2::Digest;
use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use serde::Serialize;


// ObjectMap的key的最大长度
pub const OBJECT_MAP_KEY_MAX_LEN: usize = 256;

// Map类型的SimpleContent，不需要转换大小的最大安全长度(key=256)
// pub const OBJECT_MAP_SIMPLE_MAP_CONTENT_SAFE_MAX_LEN: usize = 220;

// Set类型的SimpleContent，不需要转换大小的最大安全长度
// pub const OBJECT_MAP_SIMPLE_SET_CONTENT_SAFE_MAX_LEN: usize = 2040;

// desc部分除去content后的预留长度，当前版本是19个bytes，预留一些空间
pub const OBJECT_MAP_DESC_FIELDS_RESERVED_SIZE: u8 = 64;

// 对象content编码后的最大长度
pub const OBJECT_MAP_CONTENT_MAX_ENCODE_SIZE: u64 =
    (u16::MAX - OBJECT_MAP_DESC_FIELDS_RESERVED_SIZE as u16) as u64;

// SUB模式下的一致性hash的固定长度
const SUB_LIST_CONST_LENGTH: u64 = 1900;

// 这是一组用来快速测试多级objectmap的配置参数
/*
// Map类型的SimpleContent，不需要转换大小的最大安全长度(key=256)
pub const OBJECT_MAP_SIMPLE_MAP_CONTENT_SAFE_MAX_LEN: usize = 3;

// Set类型的SimpleContent，不需要转换大小的最大安全长度
pub const OBJECT_MAP_SIMPLE_SET_CONTENT_SAFE_MAX_LEN: usize = 3;

// 对象编码后的最大长度
pub const OBJECT_MAP_CONTENT_MAX_ENCODE_SIZE: u64 = 200;

// SUB模式下的一致性hash的固定长度
const SUB_LIST_CONST_LENGTH: u64 = 2;
*/


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObjectMapCreateStrategy {
    NotCreate = 0,
    CreateNew = 1,
    CreateIfNotExists = 2,
}

pub type ObjectMapRef = Arc<AsyncMutex<ObjectMap>>;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct MapContentT<T>
where
    T: Send
        + Sync
        + Clone
        + Eq
        + PartialEq
        + std::fmt::Display
        + RawEncode
        + IntoObjectMapContentItem,
{
    values: BTreeMap<String, T>,

    #[cyfs(skip)]
    dirty: bool,
}

impl<T> MapContentT<T>
where
    T: Send
        + Sync
        + Clone
        + Eq
        + PartialEq
        + std::fmt::Display
        + RawEncode
        + IntoObjectMapContentItem,
{
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
            dirty: false,
        }
    }

    pub fn values(&self) -> &BTreeMap<String, T> {
        &self.values
    }

    pub fn into_values(self) -> BTreeMap<String, T> {
        self.values
    }

    pub fn merge(&mut self, other: Self) -> BuckyResult<()> {
        for (key, value) in other.into_values() {
            match self.values.entry(key.clone()) {
                Entry::Vacant(v) => {
                    v.insert(value);
                    self.dirty = true;
                }
                Entry::Occupied(_o) => {
                    let msg = format!("merge ObjectMap with map content error! object with key already exists! key={}", key);
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
                }
            }
        }

        Ok(())
    }

    pub fn list(&self, list: &mut ObjectMapContentList) -> BuckyResult<usize> {
        let mut len = 0;
        for (key, value) in &self.values {
            list.push_key_value(key, value.to_owned());
            len += 1;
        }

        Ok(len)
    }

    pub fn next(&self, it: &mut ObjectMapIterator) -> BuckyResult<()> {
        // 读取当前的状态
        let pos = it.current_pos().into_simple_map();
        // info!("current pos: {:?}", pos);

        let begin = match pos {
            Some(key) => std::ops::Bound::Excluded(key),
            None => std::ops::Bound::Unbounded,
        };

        let end = std::ops::Bound::Unbounded;

        let range = self.values.range((begin, end));
        for (key, value) in range {
            it.push_key_value(key, value.to_owned());
            if it.is_enough() {
                // 当前对象还没迭代完，那么需要保存状态
                it.update_pos(
                    it.depth(),
                    IteratorPosition::SimpleMap(Some(key.to_owned())),
                );
                break;
            }
        }

        Ok(())
    }

    pub(crate) fn diff(&self, other: &MapContentT<T>, diff: &mut ObjectMapDiff) {
        for (key, value) in &self.values {
            if let Some(other_value) = other.values.get(key) {
                if value != other_value {
                    diff.map_alter_item(&key, value.to_owned(), other_value.to_owned());
                }
            } else {
                diff.map_item(ObjectMapDiffAction::Remove, &key, value.to_owned());
            }
        }

        for (key, value) in &other.values {
            if let None = self.values.get(key) {
                diff.map_item(ObjectMapDiffAction::Add, &key, value.to_owned());
            }
        }
    }

    // map methods
    pub fn get_by_key(&self, key: &str) -> BuckyResult<Option<T>> {
        match self.values.get(key) {
            Some(v) => Ok(Some(v.to_owned())),
            None => Ok(None),
        }
    }

    pub fn insert_with_key(&mut self, key: &str, value: &T) -> BuckyResult<()> {
        match self.values.entry(key.to_owned()) {
            Entry::Vacant(v) => {
                v.insert(value.to_owned());
                self.dirty = true;
                Ok(())
            }
            Entry::Occupied(_o) => {
                let msg = format!("object with key already exists! key={}", key);
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg))
            }
        }
    }

    pub fn set_with_key(
        &mut self,
        key: &str,
        value: &T,
        prev_value: &Option<T>,
        auto_insert: bool,
    ) -> BuckyResult<Option<T>> {
        match self.values.entry(key.to_owned()) {
            Entry::Vacant(v) => {
                if auto_insert {
                    debug!(
                        "set_with_key auto insert new value: key={}, value={}",
                        key, value
                    );

                    v.insert(value.to_owned());
                    self.dirty = true;
                    Ok(None)
                } else {
                    let msg = format!("set_with_key but not found! key={}", key);
                    warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                }
            }
            Entry::Occupied(o) => {
                let o = o.into_mut();
                let old = o.clone();

                // 如果指定了前一个值，那么使用CAS操作
                if let Some(prev_value) = prev_value {
                    if *prev_value != old {
                        let msg = format!(
                            "set_with_key but not match! key={}, now={}, prev={}",
                            key, old, prev_value
                        );
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                    }
                }

                if *value != old {
                    debug!("set_with_key: key={}, {} -> {}", key, old, value);
                    *o = value.to_owned();
                    self.dirty = true;
                }

                Ok(Some(old))
            }
        }
    }

    pub fn remove_with_key(&mut self, key: &str, prev_value: &Option<T>) -> BuckyResult<Option<T>> {
        match prev_value {
            Some(prev_value) => {
                // 如果指定了前值，那么需要判断下
                match self.values.get(key) {
                    Some(current) => {
                        if *current != *prev_value {
                            let msg = format!(
                                "remove_with_key but not match! key={}, now={}, prev={}",
                                key, current, prev_value
                            );
                            warn!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                        }

                        let ret = self.values.remove(key).unwrap();
                        self.dirty = true;
                        Ok(Some(ret))
                    }
                    None => {
                        // 如果已经不存在了，那么不需要和prev_value匹配了，直接返回成功
                        Ok(None)
                    }
                }
            }
            None => {
                let ret = self.values.remove(key);
                if ret.is_some() {
                    self.dirty = true;
                }
                Ok(ret)
            }
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SetContentT<T>
where
    T: Send
        + Sync
        + Clone
        + Ord
        + std::fmt::Display
        + RawEncode
        + IntoObjectMapContentItem
        + From<SetIteratorPostion>,
{
    values: BTreeSet<T>,

    #[cyfs(skip)]
    dirty: bool,
}

impl<T> SetContentT<T>
where
    T: Send
        + Sync
        + Clone
        + Ord
        + std::fmt::Display
        + RawEncode
        + IntoObjectMapContentItem
        + From<SetIteratorPostion>,
    SetIteratorPostion: From<T>,
{
    pub fn new() -> Self {
        Self {
            values: BTreeSet::new(),
            dirty: false,
        }
    }

    pub fn values(&self) -> &BTreeSet<T> {
        &self.values
    }

    pub fn into_values(self) -> BTreeSet<T> {
        self.values
    }

    pub fn merge(&mut self, other: Self) -> BuckyResult<()> {
        for value in other.into_values() {
            match self.values.insert(value.clone()) {
                true => {
                    self.dirty = true;
                    continue;
                }
                false => {
                    let msg = format!(
                        "merge ObjectMap with set content error! object already exists! value={}",
                        value
                    );
                    warn!("{}", msg);
                }
            }
        }

        Ok(())
    }

    pub fn list(&self, list: &mut ObjectMapContentList) -> BuckyResult<usize> {
        let mut len = 0;
        for value in &self.values {
            list.push_value(value.to_owned());
            len += 1;
        }

        Ok(len)
    }

    pub fn next(&self, it: &mut ObjectMapIterator) -> BuckyResult<()> {
        // 读取当前的状态
        let pos = it.current_pos().into_simple_set();

        let begin: std::ops::Bound<T> = match pos {
            Some(key) => std::ops::Bound::Excluded(key.into()),
            None => std::ops::Bound::Unbounded,
        };

        let end = std::ops::Bound::Unbounded;

        let range = self.values.range((begin, end));
        for value in range {
            it.push_value(value.to_owned());
            if it.is_enough() {
                // 当前对象还没迭代完，那么需要保存状态
                let pos = SetIteratorPostion::from(value.to_owned());
                it.update_pos(it.depth(), IteratorPosition::SimpleSet(Some(pos)));
                break;
            }
        }

        Ok(())
    }

    pub(crate) fn diff(&self, other: &SetContentT<T>, diff: &mut ObjectMapDiff) {
        for value in &self.values {
            if let None = other.values.get(value) {
                diff.set_item(ObjectMapDiffAction::Remove, value.to_owned());
            }
        }

        for value in &other.values {
            if let None = self.values.get(value) {
                diff.set_item(ObjectMapDiffAction::Add, value.to_owned());
            }
        }
    }

    // set methods
    pub fn contains(&self, value: &T) -> BuckyResult<bool> {
        Ok(self.values.contains(value))
    }

    pub fn insert(&mut self, value: &T) -> BuckyResult<bool> {
        let ret = self.values.insert(value.to_owned());
        if ret {
            self.dirty = true;
        }

        Ok(ret)
    }

    pub fn remove(&mut self, value: &T) -> BuckyResult<bool> {
        let ret = self.values.remove(value);
        if ret {
            self.dirty = true;
        }

        Ok(ret)
    }
}

type MapContent = MapContentT<ObjectId>;
type DiffMapContent = MapContentT<ObjectMapDiffMapItem>;

type SetContent = SetContentT<ObjectId>;
type DiffSetContent = SetContentT<ObjectMapDiffSetItem>;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum SimpleContent {
    Map(MapContent),
    DiffMap(DiffMapContent),
    Set(SetContent),
    DiffSet(DiffSetContent),
}

impl SimpleContent {
    pub fn content_type(&self) -> ObjectMapSimpleContentType {
        match &self {
            Self::Map(_) => ObjectMapSimpleContentType::Map,
            Self::DiffMap(_) => ObjectMapSimpleContentType::DiffMap,
            Self::Set(_) => ObjectMapSimpleContentType::Set,
            Self::DiffSet(_) => ObjectMapSimpleContentType::DiffSet,
        }
    }

    pub fn as_map(&self) -> &MapContent {
        match &self {
            Self::Map(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_diff_map(&self) -> &DiffMapContent {
        match &self {
            Self::DiffMap(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_set(&self) -> &SetContent {
        match &self {
            Self::Set(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_diff_set(&self) -> &DiffSetContent {
        match &self {
            Self::DiffSet(value) => value,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ObjectMapSimpleContent {
    depth: u8,
    content: SimpleContent,
}

impl ObjectMapSimpleContent {
    pub fn new(content_type: ObjectMapSimpleContentType, depth: u8) -> Self {
        let content = match content_type {
            ObjectMapSimpleContentType::Map => SimpleContent::Map(MapContent::new()),
            ObjectMapSimpleContentType::DiffMap => SimpleContent::DiffMap(DiffMapContent::new()),
            ObjectMapSimpleContentType::Set => SimpleContent::Set(SetContent::new()),
            ObjectMapSimpleContentType::DiffSet => SimpleContent::DiffSet(DiffSetContent::new()),
        };

        Self { depth, content }
    }

    pub fn content(&self) -> &SimpleContent {
        &self.content
    }

    pub fn len(&self) -> usize {
        match &self.content {
            SimpleContent::Map(content) => content.values.len(),
            SimpleContent::DiffMap(content) => content.values.len(),
            SimpleContent::Set(content) => content.values.len(),
            SimpleContent::DiffSet(content) => content.values.len(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        match &self.content {
            SimpleContent::Map(content) => content.dirty,
            SimpleContent::DiffMap(content) => content.dirty,
            SimpleContent::Set(content) => content.dirty,
            SimpleContent::DiffSet(content) => content.dirty,
        }
    }

    pub fn clear_dirty(&mut self) {
        match &mut self.content {
            SimpleContent::Map(content) => content.dirty = false,
            SimpleContent::DiffMap(content) => content.dirty = false,
            SimpleContent::Set(content) => content.dirty = false,
            SimpleContent::DiffSet(content) => content.dirty = false,
        }
    }

    pub fn merge(&mut self, other: Self) -> BuckyResult<()> {
        match &mut self.content {
            SimpleContent::Map(content) => match other.content {
                SimpleContent::Map(other) => {
                    content.merge(other)?;
                }
                _ => {
                    let msg =
                        format!("merge ObjectMap with map content error! unmatch content type");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }
            },
            SimpleContent::DiffMap(content) => match other.content {
                SimpleContent::DiffMap(other) => {
                    content.merge(other)?;
                }
                _ => {
                    let msg =
                        format!("merge ObjectMap with diffmap content error! unmatch content type");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }
            },

            SimpleContent::Set(content) => match other.content {
                SimpleContent::Set(other) => {
                    content.merge(other)?;
                }
                _ => {
                    let msg =
                        format!("merge ObjectMap with set content error! unmatch content type");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }
            },
            SimpleContent::DiffSet(content) => match other.content {
                SimpleContent::DiffSet(other) => {
                    content.merge(other)?;
                }
                _ => {
                    let msg =
                        format!("merge ObjectMap with diffet content error! unmatch content type");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }
            },
        }

        Ok(())
    }

    // 用以对基于path的多级objectmap的支持
    pub async fn get_or_create_child_object_map(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        auto_create: ObjectMapCreateStrategy,
    ) -> BuckyResult<Option<ObjectMapRef>> {
        let ret = self.get_by_key(key)?;
        match ret {
            Some(sub_id) => {
                if auto_create == ObjectMapCreateStrategy::CreateNew {
                    let msg = format!(
                        "objectmap solt already been taken! key={}, current={}, type={:?}",
                        key, sub_id, sub_id.obj_type_code(),
                    );
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
                }

                // 如果子对象类型不是objectmap
                if sub_id.obj_type_code() != ObjectTypeCode::ObjectMap {
                    let msg = format!(
                        "objectmap solt already been taken by other object! key={}, current={}, type={:?}",
                        key, sub_id, sub_id.obj_type_code(),
                    );
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
                }

                // 加载对应的submap
                let sub_map = cache.get_object_map(&sub_id).await?;
                if sub_map.is_none() {
                    let msg = format!(
                        "get sub objectmap from cache but not found! key={}, id={}",
                        key, sub_id
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }

                Ok(Some(sub_map.unwrap()))
            }
            None => {
                if auto_create == ObjectMapCreateStrategy::NotCreate {
                    return Ok(None);
                }

                // 创建新的object map
                let sub = builder.clone().class(ObjectMapClass::Root).build();
                let sub_id = sub.flush_id();
                if let Err(e) = self.insert_with_key(key, &sub_id) {
                    let msg = format!("insert object map with key={} error! {}", key, e);
                    error!("{}", msg);
                    return Err(BuckyError::new(e.code(), msg));
                }

                debug!("object map new sub: key={}, sub={}", key, sub_id);
                let sub_map = cache.put_object_map(&sub_id, sub)?;
                Ok(Some(sub_map))
            }
        }
    }

    pub fn list(&self, list: &mut ObjectMapContentList) -> BuckyResult<usize> {
        match &self.content {
            SimpleContent::Map(content) => content.list(list),
            SimpleContent::DiffMap(content) => content.list(list),
            SimpleContent::Set(content) => content.list(list),
            SimpleContent::DiffSet(content) => content.list(list),
        }
    }

    pub fn next(&self, it: &mut ObjectMapIterator) -> BuckyResult<()> {
        match &self.content {
            SimpleContent::Map(content) => content.next(it),
            SimpleContent::DiffMap(content) => content.next(it),
            SimpleContent::Set(content) => content.next(it),
            SimpleContent::DiffSet(content) => content.next(it),
        }
    }

    pub(crate) fn diff(&self, other: &Self, diff: &mut ObjectMapDiff) {
        match &self.content {
            SimpleContent::Map(content) => content.diff(other.content.as_map(), diff),
            SimpleContent::DiffMap(content) => content.diff(other.content.as_diff_map(), diff),
            SimpleContent::Set(content) => content.diff(other.content.as_set(), diff),
            SimpleContent::DiffSet(content) => content.diff(other.content.as_diff_set(), diff),
        }
    }

    // visitor
    pub(crate) async fn visit(&self, visitor: &mut impl ObjectMapVisitor) -> BuckyResult<()> {
        match &self.content {
            SimpleContent::Map(content) => {
                for (key, value) in &content.values {
                    visitor.visit_map_item(&key, value).await?;
                }
            }
            SimpleContent::DiffMap(content) => {
                for (key, value) in &content.values {
                    visitor.visit_diff_map_item(&key, value).await?;
                }
            }
            SimpleContent::Set(content) => {
                for value in &content.values {
                    visitor.visit_set_item(value).await?;
                }
            }
            SimpleContent::DiffSet(content) => {
                for value in &content.values {
                    visitor.visit_diff_set_item(value).await?;
                }
            }
        }

        Ok(())
    }

    // map methods
    pub fn get_by_key(&self, key: &str) -> BuckyResult<Option<ObjectId>> {
        match &self.content {
            SimpleContent::Map(content) => content.get_by_key(key),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, key={}",
                    self.content.content_type(),
                    key
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn insert_with_key(&mut self, key: &str, value: &ObjectId) -> BuckyResult<()> {
        match &mut self.content {
            SimpleContent::Map(content) => content.insert_with_key(key, value),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, key={}",
                    self.content.content_type(),
                    key
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn set_with_key(
        &mut self,
        key: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        match &mut self.content {
            SimpleContent::Map(content) => {
                content.set_with_key(key, value, prev_value, auto_insert)
            }
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, key={}",
                    self.content.content_type(),
                    key
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn remove_with_key(
        &mut self,
        key: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        match &mut self.content {
            SimpleContent::Map(content) => content.remove_with_key(key, prev_value),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, key={}",
                    self.content.content_type(),
                    key
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    // diffmap methods
    pub fn diff_get_by_key(&self, key: &str) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        match &self.content {
            SimpleContent::DiffMap(content) => content.get_by_key(key),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, key={}",
                    self.content.content_type(),
                    key
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn diff_insert_with_key(
        &mut self,
        key: &str,
        value: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        match &mut self.content {
            SimpleContent::DiffMap(content) => content.insert_with_key(key, value),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, key={}",
                    self.content.content_type(),
                    key
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn diff_set_with_key(
        &mut self,
        key: &str,
        value: &ObjectMapDiffMapItem,
        prev_value: &Option<ObjectMapDiffMapItem>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        match &mut self.content {
            SimpleContent::DiffMap(content) => {
                content.set_with_key(key, value, prev_value, auto_insert)
            }
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, key={}",
                    self.content.content_type(),
                    key
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn diff_remove_with_key(
        &mut self,
        key: &str,
        prev_value: &Option<ObjectMapDiffMapItem>,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        match &mut self.content {
            SimpleContent::DiffMap(content) => content.remove_with_key(key, prev_value),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, key={}",
                    self.content.content_type(),
                    key
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    // set methods
    pub fn contains(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        match &self.content {
            SimpleContent::Set(content) => content.contains(object_id),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, object={}",
                    self.content.content_type(),
                    object_id,
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn insert(&mut self, object_id: &ObjectId) -> BuckyResult<bool> {
        match &mut self.content {
            SimpleContent::Set(content) => content.insert(object_id),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, object={}",
                    self.content.content_type(),
                    object_id,
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn remove(&mut self, object_id: &ObjectId) -> BuckyResult<bool> {
        match &mut self.content {
            SimpleContent::Set(content) => content.remove(object_id),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, object={}",
                    self.content.content_type(),
                    object_id,
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    // diff set methods
    pub fn diff_contains(&self, object_id: &ObjectMapDiffSetItem) -> BuckyResult<bool> {
        match &self.content {
            SimpleContent::DiffSet(content) => content.contains(object_id),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, object={}",
                    self.content.content_type(),
                    object_id,
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn diff_insert(&mut self, object_id: &ObjectMapDiffSetItem) -> BuckyResult<bool> {
        match &mut self.content {
            SimpleContent::DiffSet(content) => content.insert(object_id),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, object={}",
                    self.content.content_type(),
                    object_id,
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn diff_remove(&mut self, object_id: &ObjectMapDiffSetItem) -> BuckyResult<bool> {
        match &mut self.content {
            SimpleContent::DiffSet(content) => content.remove(object_id),
            _ => {
                let msg = format!(
                    "unmatch objectmap content type: {:?}, object={}",
                    self.content.content_type(),
                    object_id,
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode, Serialize)]
pub struct ObjectMapHubItem {
    id: ObjectId,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ObjectMapHubContent {
    depth: u8,
    subs: BTreeMap<u16, ObjectMapHubItem>,

    #[cyfs(skip)]
    dirty: bool,
}

impl ObjectMapHubContent {
    pub fn new(depth: u8) -> Self {
        Self {
            depth,
            subs: BTreeMap::new(),
            dirty: false,
        }
    }

    pub fn subs(&self) -> &BTreeMap<u16, ObjectMapHubItem> {
        &self.subs
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    fn hash_bytes(&self, key: &[u8]) -> u16 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        let mut sha256 = sha2::Sha256::new();
        sha256.input(key);
        sha256.input([self.depth]);
        hasher.write(&sha256.result());
        let index = (hasher.finish() % SUB_LIST_CONST_LENGTH) as u16;

        /*
        let s = String::from_utf8(key.to_vec()).unwrap();
        trace!(
            "hash sub index: depth={}, key={}, index={}",
            self.depth,
            s,
            index
        );
        */

        index
    }

    async fn get_sub(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &[u8],
    ) -> BuckyResult<Option<ObjectMapRef>> {
        let index = self.hash_bytes(key);
        match self.subs.get(&index) {
            Some(sub) => cache.get_object_map(&sub.id).await,
            None => Ok(None),
        }
    }

    async fn get_sub_mut(
        &mut self,
        builder: Option<&ObjectMapBuilder>,
        cache: &ObjectMapOpEnvCacheRef,
        key: &[u8],
        auto_create: bool,
    ) -> BuckyResult<Option<(&mut ObjectMapHubItem, ObjectMap)>> {
        let index = self.hash_bytes(key);
        match self.subs.entry(index) {
            Entry::Occupied(o) => {
                let sub = o.into_mut();
                match cache.get_object_map(&sub.id).await? {
                    Some(item) => Ok(Some((sub, item.lock().await.clone()))),
                    None => {
                        let msg = format!("objectmap sub item exists but load from noc not found! sub={}, key={:?}, index={}", sub.id, key, index);
                        error!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                    }
                }
            }
            Entry::Vacant(v) => {
                if auto_create {
                    // 创建子对象
                    let builder = builder.unwrap().clone().class(ObjectMapClass::Sub);
                    let sub = builder.build();
                    let id = sub.flush_id();

                    let item = ObjectMapHubItem { id };
                    let solt = v.insert(item);
                    self.dirty = true;

                    // 这里不再进行一次空map的保存，外部在操作完毕后需要调用put来保存
                    // let sub = cache.put_object_map(sub)?;
                    Ok(Some((solt, sub)))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn remove_sub(&mut self, _cache: &ObjectMapOpEnvCacheRef, key: &[u8]) -> Option<ObjectId> {
        let index = self.hash_bytes(key);
        match self.subs.remove(&index) {
            Some(sub) => Some(sub.id),
            None => None,
        }
    }

    // 目前id都是实时计算

    // path methods
    #[async_recursion::async_recursion]
    pub async fn get_or_create_child_object_map(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        auto_create: ObjectMapCreateStrategy,
    ) -> BuckyResult<Option<ObjectMapRef>> {
        let sub_auto_create = match auto_create {
            ObjectMapCreateStrategy::NotCreate => false,
            _ => true,
        };

        let sub = self
            .get_sub_mut(Some(builder), cache, key.as_bytes(), sub_auto_create)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                // 复制后修改
                let ret = obj_map
                    .get_or_create_child_object_map(cache, key, builder.content_type(), auto_create)
                    .await?;
                let current_id = obj_map.cached_object_id();
                let new_id = obj_map.flush_id();

                // 只有objectmap的id改变了，才需要保存并更新
                if current_id != Some(new_id) {
                    cache.put_object_map(&new_id, obj_map)?;
                    solt.id = new_id;
                    self.dirty = true;
                }
                Ok(ret)
            }
            None => Ok(None),
        }
    }

    #[async_recursion::async_recursion]
    pub async fn list(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        list: &mut ObjectMapContentList,
    ) -> BuckyResult<u64> {
        let mut total: u64 = 0;
        for (_key, sub) in &self.subs {
            let sub_map = cache.get_object_map(&sub.id).await?;
            if sub_map.is_none() {
                let msg = format!(
                    "read objectmap to list but sub item not found: id={}",
                    sub.id
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
            let sub_map = sub_map.unwrap();
            let obj = sub_map.lock().await;
            total += obj.list(cache, list).await? as u64;
        }

        Ok(total)
    }

    #[async_recursion::async_recursion]
    pub async fn list_subs(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        list: &mut Vec<ObjectId>,
    ) -> BuckyResult<u64> {
        let mut total: u64 = 0;
        for (_key, sub) in &self.subs {
            list.push(sub.id.clone());

            let sub_map = cache.get_object_map(&sub.id).await?;
            if sub_map.is_none() {
                let msg = format!(
                    "read objectmap to list but sub item not found: id={}",
                    sub.id
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
            let sub_map = sub_map.unwrap();
            let obj = sub_map.lock().await;
            total += obj.list_subs(cache, list).await? as u64;
        }

        Ok(total)
    }

    #[async_recursion::async_recursion]
    pub async fn next(&self, it: &mut ObjectMapIterator) -> BuckyResult<()> {
        // 读取当前的状态
        let pos = it.current_pos().into_hub();
        let depth = it.depth();

        let begin = match pos {
            Some(key) => std::ops::Bound::Included(key),
            None => std::ops::Bound::Unbounded,
        };

        let end = std::ops::Bound::Unbounded;

        let range = self.subs.range((begin, end));
        for (key, sub) in range {
            let sub_map = it.cache.get_object_map(&sub.id).await?;
            if sub_map.is_none() {
                let msg = format!(
                    "load sub object map from cache but not found! id={}",
                    sub.id
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
            let sub_map = sub_map.unwrap();
            {
                let obj = sub_map.lock().await;
                it.inc_depth(obj.mode(), obj.content_type());

                obj.next(it).await?;
            }
            if it.is_enough() {
                // 当前对象还没迭代完，那么需要保存状态
                it.update_pos(depth, IteratorPosition::Hub(Some(key.to_owned())));

                return Ok(());
            }

            // 继续下一个sub
            it.dec_depth();
        }

        assert!(!it.is_enough());

        Ok(())
    }

    pub(crate) fn diff(&self, other: &Self, diff: &mut ObjectMapDiff) {
        for (key, value) in &self.subs {
            if let Some(other_value) = other.subs.get(key) {
                if value.id != other_value.id {
                    diff.pend_async_sub_alter(&value.id, &other_value.id);
                }
            } else {
                diff.pend_async_remove(&value.id);
            }
        }

        for (key, value) in &other.subs {
            if let None = self.subs.get(key) {
                diff.pend_async_add(&value.id);
            }
        }
    }

    // visitor
    pub async fn visit(&self, visitor: &mut impl ObjectMapVisitor) -> BuckyResult<()> {
        for (_key, value) in &self.subs {
            visitor.visit_hub_item(&value.id).await?;
        }

        Ok(())
    }

    // map methods
    #[async_recursion::async_recursion]
    pub async fn get_by_key(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
    ) -> BuckyResult<Option<ObjectId>> {
        let sub = self.get_sub(cache, key.as_bytes()).await?;
        match sub {
            Some(sub) => {
                let obj_map = sub.lock().await;
                obj_map.get_by_key(cache, key).await
            }
            None => Ok(None),
        }
    }

    #[async_recursion::async_recursion]
    pub async fn insert_with_key(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectId,
    ) -> BuckyResult<()> {
        let sub = self
            .get_sub_mut(Some(builder), cache, key.as_bytes(), true)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                obj_map.insert_with_key(&cache, &key, &value).await?;

                let new_id = obj_map.flush_id();
                // debug!("insert_with_key, sub objectmap updated: key={}, {:?} -> {}", key, obj_map.cached_object_id(), new_id);
                cache.put_object_map(&new_id, obj_map)?;
                solt.id = new_id;
                self.dirty = true;
                Ok(())
            }
            None => {
                unreachable!();
            }
        }
    }

    #[async_recursion::async_recursion]
    pub async fn set_with_key(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let sub = self
            .get_sub_mut(Some(builder), cache, key.as_bytes(), true)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                let ret = obj_map
                    .set_with_key(cache, key, value, prev_value, auto_insert)
                    .await?;
                let current_id = obj_map.cached_object_id();
                let new_id = obj_map.flush_id();
                if current_id != Some(new_id) {
                    cache.put_object_map(&new_id, obj_map)?;
                    solt.id = new_id;
                    self.dirty = true;
                }
                Ok(ret)
            }
            None => {
                unreachable!();
            }
        }
    }

    #[async_recursion::async_recursion]
    pub async fn remove_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        let sub = self.get_sub_mut(None, cache, key.as_bytes(), false).await?;
        match sub {
            Some((solt, mut obj_map)) => {
                let ret = obj_map.remove_with_key(cache, key, prev_value).await?;

                if ret.is_some() {
                    if obj_map.count() > 0 {
                        let new_id = obj_map.flush_id();
                        cache.put_object_map(&new_id, obj_map)?;
                        solt.id = new_id;
                    } else {
                        debug!(
                            "sub objectmap is empty! now will remove: key={}, sub={}",
                            key, solt.id
                        );
                        let sub_id = solt.id.clone();
                        drop(solt);
                        drop(obj_map);

                        let ret = self.remove_sub(cache, key.as_bytes());
                        assert_eq!(ret, Some(sub_id));
                    }
                    self.dirty = true;
                }
                Ok(ret)
            }
            None => {
                unreachable!();
            }
        }
    }

    // diff map methods
    #[async_recursion::async_recursion]
    pub async fn diff_get_by_key(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        let sub = self.get_sub(cache, key.as_bytes()).await?;
        match sub {
            Some(sub) => {
                let obj_map = sub.lock().await;
                obj_map.diff_get_by_key(cache, key).await
            }
            None => Ok(None),
        }
    }

    #[async_recursion::async_recursion]
    pub async fn diff_insert_with_key(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        let sub = self
            .get_sub_mut(Some(builder), cache, key.as_bytes(), true)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                obj_map.diff_insert_with_key(&cache, &key, &value).await?;

                let new_id = obj_map.flush_id();
                // debug!("insert_with_key, sub objectmap updated: key={}, {:?} -> {}", key, obj_map.cached_object_id(), new_id);
                cache.put_object_map(&new_id, obj_map)?;
                solt.id = new_id;
                self.dirty = true;
                Ok(())
            }
            None => {
                unreachable!();
            }
        }
    }

    #[async_recursion::async_recursion]
    pub async fn diff_set_with_key(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectMapDiffMapItem,
        prev_value: &Option<ObjectMapDiffMapItem>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        let sub = self
            .get_sub_mut(Some(builder), cache, key.as_bytes(), true)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                let ret = obj_map
                    .diff_set_with_key(cache, key, value, prev_value, auto_insert)
                    .await?;
                let current_id = obj_map.cached_object_id();
                let new_id = obj_map.flush_id();
                if current_id != Some(new_id) {
                    cache.put_object_map(&new_id, obj_map)?;
                    solt.id = new_id;
                    self.dirty = true;
                }
                Ok(ret)
            }
            None => {
                unreachable!();
            }
        }
    }

    #[async_recursion::async_recursion]
    pub async fn diff_remove_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        prev_value: &Option<ObjectMapDiffMapItem>,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        let sub = self.get_sub_mut(None, cache, key.as_bytes(), false).await?;
        match sub {
            Some((solt, mut obj_map)) => {
                let ret = obj_map.diff_remove_with_key(cache, key, prev_value).await?;

                if ret.is_some() {
                    if obj_map.count() > 0 {
                        let new_id = obj_map.flush_id();
                        cache.put_object_map(&new_id, obj_map)?;
                        solt.id = new_id;
                    } else {
                        debug!(
                            "sub objectmap is empty! now will remove: key={}, sub={}",
                            key, solt.id
                        );
                        let sub_id = solt.id.clone();
                        drop(solt);
                        drop(obj_map);

                        let ret = self.remove_sub(cache, key.as_bytes());
                        assert_eq!(ret, Some(sub_id));
                    }
                    self.dirty = true;
                }
                Ok(ret)
            }
            None => {
                unreachable!();
            }
        }
    }

    // set methods
    #[async_recursion::async_recursion]
    pub async fn contains(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        let sub = self.get_sub(cache, object_id.as_ref().as_slice()).await?;
        match sub {
            Some(sub) => {
                let obj_map = sub.lock().await;
                obj_map.contains(cache, object_id).await
            }
            None => Ok(false),
        }
    }

    #[async_recursion::async_recursion]
    pub async fn insert(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        let sub = self
            .get_sub_mut(Some(builder), cache, object_id.as_ref().as_slice(), true)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                let ret = obj_map.insert(cache, object_id).await?;
                if ret {
                    let new_id = obj_map.flush_id();
                    cache.put_object_map(&new_id, obj_map)?;
                    solt.id = new_id;
                    self.dirty = true;
                }

                Ok(ret)
            }
            None => {
                unreachable!();
            }
        }
    }

    #[async_recursion::async_recursion]
    pub async fn remove(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        let sub = self
            .get_sub_mut(None, cache, object_id.as_ref().as_slice(), false)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                let ret = obj_map.remove(cache, object_id).await?;
                if ret {
                    if obj_map.count() > 0 {
                        let new_id = obj_map.flush_id();
                        cache.put_object_map(&new_id, obj_map)?;
                        solt.id = new_id;
                    } else {
                        debug!(
                            "sub objectmap is empty! now will remove: value={}, sub={}",
                            object_id, solt.id
                        );
                        drop(solt);
                        drop(obj_map);

                        let _ret = self.remove_sub(cache, object_id.as_ref().as_slice());
                    }
                    self.dirty = true;
                }

                Ok(ret)
            }
            None => {
                let msg = format!(
                    "remove object from objectmap with set content but sub obj not found! object={}",
                    object_id
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    // diff set methods
    #[async_recursion::async_recursion]
    pub async fn diff_contains(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        let sub = self.get_sub(cache, object_id.as_slice()).await?;
        match sub {
            Some(sub) => {
                let obj_map = sub.lock().await;
                obj_map.diff_contains(cache, object_id).await
            }
            None => Ok(false),
        }
    }

    #[async_recursion::async_recursion]
    pub async fn diff_insert(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        let sub = self
            .get_sub_mut(Some(builder), cache, object_id.as_slice(), true)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                let ret = obj_map.diff_insert(cache, object_id).await?;
                if ret {
                    let new_id = obj_map.flush_id();
                    cache.put_object_map(&new_id, obj_map)?;
                    solt.id = new_id;
                    self.dirty = true;
                }

                Ok(ret)
            }
            None => {
                unreachable!();
            }
        }
    }

    #[async_recursion::async_recursion]
    pub async fn diff_remove(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        let sub = self
            .get_sub_mut(None, cache, object_id.as_slice(), false)
            .await?;
        match sub {
            Some((solt, mut obj_map)) => {
                let ret = obj_map.diff_remove(cache, object_id).await?;
                if ret {
                    if obj_map.count() > 0 {
                        let new_id = obj_map.flush_id();
                        cache.put_object_map(&new_id, obj_map)?;
                        solt.id = new_id;
                    } else {
                        debug!(
                            "sub objectmap is empty! now will remove: value={}, sub={}",
                            object_id, solt.id
                        );
                        drop(solt);
                        drop(obj_map);

                        let _ret = self.remove_sub(cache, object_id.as_slice());
                    }
                    self.dirty = true;
                }

                Ok(ret)
            }
            None => {
                let msg = format!(
                    "remove object from objectmap with set content but sub obj not found! object={}",
                    object_id
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }
}

#[derive(Clone, Debug, Copy, RawEncode, RawDecode, Eq, PartialEq)]
pub enum ObjectMapSimpleContentType {
    Map,
    DiffMap,
    Set,
    DiffSet,
}

impl ObjectMapSimpleContentType {
    pub fn get_diff_type(&self) -> Option<ObjectMapSimpleContentType> {
        match &self {
            Self::Map => Some(Self::DiffMap),
            Self::Set => Some(Self::DiffSet),
            _ => {
                // 目前diffmap和diffset不再支持diff操作
                None
            }
        }
    }

    pub fn is_diff_match(&self, diff_content_type: &Self) -> bool {
        match self.get_diff_type() {
            Some(diff_type) => diff_type == *diff_content_type,
            None => false,
        }
    }
}

impl ToString for ObjectMapSimpleContentType {
    fn to_string(&self) -> String {
        match *self {
            Self::Map => "map",
            Self::DiffMap => "diffmap",
            Self::Set => "set",
            Self::DiffSet => "diffset",
        }
        .to_owned()
    }
}

impl std::str::FromStr for ObjectMapSimpleContentType {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "map" => Self::Map,
            "diffmap" => Self::DiffMap,
            "set" => Self::Set,
            "diffset" => Self::DiffSet,

            v @ _ => {
                let msg = format!("unknown simple content type: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObjectMapContentMode {
    Simple,
    Hub,
}

impl ToString for ObjectMapContentMode {
    fn to_string(&self) -> String {
        match *self {
            Self::Simple => "simple",
            Self::Hub => "hub",
        }
        .to_owned()
    }
}

impl std::str::FromStr for ObjectMapContentMode {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "simple" => Self::Simple,
            "hub" => Self::Hub,

            v @ _ => {
                let msg = format!("unknown ObjectMapContentMode value: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum ObjectMapContent {
    Simple(ObjectMapSimpleContent),
    Hub(ObjectMapHubContent),
}

impl ObjectMapContent {
    pub fn mode(&self) -> ObjectMapContentMode {
        match self {
            Self::Simple(_) => ObjectMapContentMode::Simple,
            Self::Hub(_) => ObjectMapContentMode::Hub,
        }
    }

    pub fn new_simple(content_type: ObjectMapSimpleContentType, depth: u8) -> Self {
        ObjectMapContent::Simple(ObjectMapSimpleContent::new(content_type, depth))
    }

    pub fn new_hub(depth: u8) -> Self {
        ObjectMapContent::Hub(ObjectMapHubContent::new(depth))
    }

    pub fn is_dirty(&self) -> bool {
        match self {
            Self::Simple(content) => content.is_dirty(),
            Self::Hub(content) => content.is_dirty(),
        }
    }

    pub fn clear_dirty(&mut self) {
        match self {
            Self::Simple(content) => content.clear_dirty(),
            Self::Hub(content) => content.clear_dirty(),
        }
    }

    /*
    // 刷新id
    pub fn flush_id(&mut self, cache: &ObjectMapOpEnvCacheRef) -> BuckyResult<()> {
        match self {
            Self::Simple(_) => Ok(()),
            Self::Hub(content) => content.flush_id(cache),
        }
    }
    */

    // 模式转换 methods
    pub fn into_simple(self) -> ObjectMapSimpleContent {
        match self {
            Self::Simple(content) => content,
            Self::Hub(_) => unreachable!(),
        }
    }

    pub async fn convert_to_hub(
        self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
    ) -> BuckyResult<Self> {
        match self {
            Self::Simple(content) => {
                let mut hub = ObjectMapHubContent::new(content.depth);
                match content.content {
                    SimpleContent::Map(values) => {
                        for (key, value) in values.into_values().into_iter() {
                            hub.insert_with_key(builder, cache, &key, &value).await?;
                        }
                    }
                    SimpleContent::DiffMap(values) => {
                        for (key, value) in values.into_values().into_iter() {
                            hub.diff_insert_with_key(builder, cache, &key, &value)
                                .await?;
                        }
                    }
                    SimpleContent::Set(values) => {
                        for value in values.into_values().into_iter() {
                            hub.insert(builder, cache, &value).await?;
                        }
                    }
                    SimpleContent::DiffSet(values) => {
                        for value in values.into_values().into_iter() {
                            hub.diff_insert(builder, cache, &value).await?;
                        }
                    }
                }
                let ret = Self::Hub(hub);
                Ok(ret)
            }
            Self::Hub(_) => Ok(self),
        }
    }

    #[async_recursion::async_recursion]
    pub async fn convert_to_simple(
        self,
        cache: &ObjectMapOpEnvCacheRef,
        content_type: ObjectMapSimpleContentType,
    ) -> BuckyResult<Self> {
        match self {
            Self::Hub(content) => {
                let mut new_content = ObjectMapSimpleContent::new(content_type, content.depth);

                // 对hub模式下的每个sub对象递归的进行转换操作
                for (_, sub) in content.subs.iter() {
                    let sub_obj = cache.get_object_map(&sub.id).await?;
                    if sub_obj.is_none() {
                        let msg = format!(
                            "convert object map to simple failed, sub obj not found! obj={}",
                            sub.id
                        );
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                    }

                    let sub_obj = sub_obj.unwrap();
                    let simple_content;
                    {
                        let mut obj = sub_obj.lock().await.clone();
                        obj.convert_to_simple(cache).await?;
                        simple_content = obj.into_simple();
                    }
                    new_content.merge(simple_content)?;
                }

                let ret = Self::Simple(new_content);
                Ok(ret)
            }
            Self::Simple(_) => Ok(self),
        }
    }

    // visitor
    pub async fn visit(&self, visitor: &mut impl ObjectMapVisitor) -> BuckyResult<()> {
        match &self {
            Self::Simple(content) => content.visit(visitor).await,
            Self::Hub(content) => content.visit(visitor).await,
        }
    }
}

// 用来缓存计算id
#[derive(Debug, Clone)]
struct ObjectMapContentHashCacheImpl {
    dirty: bool,

    // 编码后的长度和内容
    // len: usize,
    // code_raw_buf: Option<Vec<u8>>,

    // 缓存的对象id
    object_id: Option<ObjectId>,
}

impl ObjectMapContentHashCacheImpl {
    pub fn new(object_id: ObjectId) -> Self {
        Self {
            dirty: false,
            object_id: Some(object_id),
        }
    }

    pub fn new_empty() -> Self {
        Self {
            dirty: true,
            object_id: None,
        }
    }

    // 设置cache的dirty状态
    fn mark_dirty(&mut self) -> bool {
        let ret = self.dirty;
        self.dirty = true;
        ret
    }
}

struct ObjectMapContentHashCache(Mutex<ObjectMapContentHashCacheImpl>);

impl Clone for ObjectMapContentHashCache {
    fn clone(&self) -> Self {
        let value = self.0.lock().unwrap().clone();
        Self(Mutex::new(value))
    }
}

impl std::fmt::Debug for ObjectMapContentHashCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.0.lock().unwrap();
        value.fmt(f)
    }
}

impl ObjectMapContentHashCache {
    pub fn new(object_id: ObjectId) -> Self {
        Self(Mutex::new(ObjectMapContentHashCacheImpl::new(object_id)))
    }

    pub fn new_empty() -> Self {
        Self(Mutex::new(ObjectMapContentHashCacheImpl::new_empty()))
    }

    pub fn mark_dirty(&self) -> bool {
        self.0.lock().unwrap().mark_dirty()
    }

    pub fn object_id(&self) -> Option<ObjectId> {
        self.0.lock().unwrap().object_id.clone()
    }

    pub fn need_flush_id(&self) -> Option<ObjectId> {
        let cache = self.0.lock().unwrap();
        if !cache.dirty {
            assert!(cache.object_id.is_some());
            Some(cache.object_id.as_ref().unwrap().to_owned())
        } else {
            None
        }
    }

    pub fn update_id(&self, object_id: &ObjectId) {
        let mut cache = self.0.lock().unwrap();
        cache.object_id = Some(object_id.clone());
        cache.dirty = false;
    }

    pub fn direct_set_id_on_init(&self, object_id: ObjectId) {
        let mut cache = self.0.lock().unwrap();
        assert!(cache.dirty);
        assert!(cache.object_id.is_none());

        cache.object_id = Some(object_id);
        cache.dirty = false;
    }
}

// ObjectMap对象的分类
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectMapClass {
    // 根节点
    Root = 0,

    // hub模式下的子节点
    Sub = 1,
}

impl ObjectMapClass {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Root => "root",
            Self::Sub => "sub",
        }
    }
}

impl Into<u8> for ObjectMapClass {
    fn into(self) -> u8 {
        unsafe { std::mem::transmute(self as u8) }
    }
}

use std::convert::TryFrom;

impl TryFrom<u8> for ObjectMapClass {
    type Error = BuckyError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let ret = match value {
            0 => Self::Root,
            1 => Self::Sub,

            _ => {
                let msg = format!("unknown objectmap class: {}", value);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

pub struct ObjectMapMetaData {
    pub content_mode: ObjectMapContentMode,
    pub content_type: ObjectMapSimpleContentType,
    pub count: u64,
    pub size: u64,
    pub depth: u8,
}

fn raw_encode_size(t: &(impl RawEncode + ?Sized)) -> u64 {
    t.raw_measure(&None).unwrap() as u64
}

// 核心对象层的实现
#[derive(Clone, Debug)]
pub struct ObjectMapDescContent {
    // 对象类别
    class: ObjectMapClass,

    // 子对象个数
    total: u64,

    // 内容总大小
    size: u64,

    // 当前深度，对于根对象，默认是0；对于sub objectmap，那么>0
    depth: u8,

    content_type: ObjectMapSimpleContentType,
    content: ObjectMapContent,

    hash_cache: ObjectMapContentHashCache,
}

impl ObjectMapDescContent {
    pub fn new(class: ObjectMapClass, content_type: ObjectMapSimpleContentType, depth: u8) -> Self {
        Self {
            class,
            total: 0,
            size: 0,
            depth,
            content_type: content_type.clone(),
            content: ObjectMapContent::new_simple(content_type, depth),
            hash_cache: ObjectMapContentHashCache::new_empty(),
        }
    }

    fn mark_dirty(&mut self) -> bool {
        // 首先重置content的dirty状态
        self.content.clear_dirty();

        // 设置cache的dirty状态
        self.hash_cache.mark_dirty()
    }

    pub fn count(&self) -> u64 {
        self.total
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn depth(&self) -> u8 {
        self.depth
    }

    pub fn object_id(&self) -> Option<ObjectId> {
        self.hash_cache.object_id()
    }

    pub fn content_type(&self) -> &ObjectMapSimpleContentType {
        &self.content_type
    }

    pub fn mode(&self) -> ObjectMapContentMode {
        self.content.mode()
    }

    pub fn class(&self) -> ObjectMapClass {
        self.class.clone()
    }

    pub fn content(&self) -> &ObjectMapContent {
        &self.content

    }
    pub fn metadata(&self) -> ObjectMapMetaData {
        ObjectMapMetaData {
            content_type: self.content_type.clone(),
            content_mode: self.content.mode(),
            count: self.total,
            size: self.size,
            depth: self.depth,
        }
    }

    // 模式转换
    pub fn into_simple(self) -> ObjectMapSimpleContent {
        self.content.into_simple()
    }

    pub async fn convert_to_simple(&mut self, cache: &ObjectMapOpEnvCacheRef) -> BuckyResult<()> {
        let mut content = ObjectMapContent::new_simple(self.content_type.clone(), self.depth);
        std::mem::swap(&mut self.content, &mut content);

        self.content = content
            .convert_to_simple(cache, self.content_type().clone())
            .await?;

        self.mark_dirty();

        Ok(())
    }

    pub async fn convert_to_hub(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
    ) -> BuckyResult<()> {
        // debug!("objectmap begin convert to hub mode, id={:?}", self.object_id());
        let mut content = ObjectMapContent::new_hub(self.depth);
        std::mem::swap(&mut self.content, &mut content);

        self.content = content.convert_to_hub(builder, cache).await?;
        self.mark_dirty();

        // debug!("objectmap end convert to hub mode, id={:?}", self.object_id());

        Ok(())
    }

    // 增加操作的检查点
    pub async fn inflate_check_point(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
    ) -> BuckyResult<()> {
        match &self.content {
            ObjectMapContent::Simple(_) => {
                // 看编码大小是否超出64k限制
                if self.size > OBJECT_MAP_CONTENT_MAX_ENCODE_SIZE {
                    info!("object map simple content extend limit, now will convert to hub mode! id={:?}, size={}, count={}", 
                        self.object_id(), self.size, self.total);
                    self.convert_to_hub(builder, cache).await?;
                }
            }
            ObjectMapContent::Hub(_) => {
                assert!(self.size > OBJECT_MAP_CONTENT_MAX_ENCODE_SIZE);
            }
        }

        Ok(())
    }

    // 减少操作的检查点
    pub async fn deflate_check_point(&mut self, cache: &ObjectMapOpEnvCacheRef) -> BuckyResult<()> {
        match &self.content {
            ObjectMapContent::Simple(_) => {
                assert!(self.size <= OBJECT_MAP_CONTENT_MAX_ENCODE_SIZE);
            }
            ObjectMapContent::Hub(_) => {
                if self.size <= OBJECT_MAP_CONTENT_MAX_ENCODE_SIZE {
                    // 转换为simple模式
                    info!("object map hub content in limit, now will convert back to simple mode! count={}", self.total);
                    self.convert_to_simple(cache).await?;
                }
            }
        }

        Ok(())
    }

    // path methods
    pub async fn get_or_create_child_object_map(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        auto_create: ObjectMapCreateStrategy,
    ) -> BuckyResult<Option<ObjectMapRef>> {
        assert!(!self.content.is_dirty());

        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => {
                content
                    .get_or_create_child_object_map(builder, cache, key, auto_create)
                    .await
            }
            ObjectMapContent::Hub(content) => {
                content
                    .get_or_create_child_object_map(builder, cache, key, auto_create)
                    .await
            }
        }?;

        // 内容发生改变，那么说明创建了新对象
        if self.content.is_dirty() {
            self.size += raw_encode_size(key) + ObjectId::raw_bytes().unwrap() as u64;
            self.total += 1;
            self.mark_dirty();
        }

        Ok(ret)
    }

    // 展开内容到列表
    pub async fn list(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        list: &mut ObjectMapContentList,
    ) -> BuckyResult<u64> {
        match &self.content {
            ObjectMapContent::Simple(content) => content.list(list).map(|v| v as u64),
            ObjectMapContent::Hub(content) => content.list(cache, list).await,
        }
    }

    pub async fn list_subs(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        list: &mut Vec<ObjectId>,
    ) -> BuckyResult<u64> {
        match &self.content {
            ObjectMapContent::Simple(_content) => Ok(0),
            ObjectMapContent::Hub(content) => content.list_subs(cache, list).await,
        }
    }

    // 使用迭代器枚举
    pub async fn next(&self, it: &mut ObjectMapIterator) -> BuckyResult<()> {
        match &self.content {
            ObjectMapContent::Simple(content) => content.next(it),
            ObjectMapContent::Hub(content) => content.next(it).await,
        }
    }

    pub(crate) fn diff(&self, other: &Self, diff: &mut ObjectMapDiff) {
        match &self.content {
            ObjectMapContent::Simple(content) => match &other.content {
                ObjectMapContent::Simple(other) => {
                    // 格式相同，直接计算diff
                    content.diff(other, diff);
                }
                ObjectMapContent::Hub(_) => {
                    unreachable!();
                }
            },
            ObjectMapContent::Hub(content) => match &other.content {
                ObjectMapContent::Simple(_) => {
                    unreachable!();
                }
                ObjectMapContent::Hub(other) => {
                    // 都是hub模式，那么递归的计算一次diff
                    content.diff(other, diff);
                }
            },
        }
    }

    // map模式的相关接口
    pub async fn get_by_key(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
    ) -> BuckyResult<Option<ObjectId>> {
        match &self.content {
            ObjectMapContent::Simple(content) => content.get_by_key(key),
            ObjectMapContent::Hub(content) => content.get_by_key(cache, key).await,
        }
    }

    pub async fn insert_with_key(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectId,
    ) -> BuckyResult<()> {
        match &mut self.content {
            ObjectMapContent::Simple(content) => content.insert_with_key(key, value),
            ObjectMapContent::Hub(content) => {
                content.insert_with_key(builder, cache, key, value).await
            }
        }?;

        self.size += raw_encode_size(key) + raw_encode_size(value);
        self.total += 1;
        self.inflate_check_point(builder, cache).await?;
        self.mark_dirty();

        debug!(
            "objectmap insert_with_key, total={}, {}={}",
            self.total, key, value
        );

        Ok(())
    }

    pub async fn set_with_key(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => {
                content.set_with_key(key, value, prev_value, auto_insert)
            }
            ObjectMapContent::Hub(content) => {
                content
                    .set_with_key(builder, cache, key, value, prev_value, auto_insert)
                    .await
            }
        }?;

        debug!(
            "objectmap set_with_key: key={}, value={}, prev={:?}, auto_insert={}, ret={:?}",
            key, value, prev_value, auto_insert, ret
        );

        if ret.is_none() {
            // 插入了一个新元素
            self.size += raw_encode_size(key) + raw_encode_size(value);
            self.total += 1;
            self.inflate_check_point(builder, cache).await?;
            self.mark_dirty();
        } else {
            // 只是发生了replace操作，元素个数和大小不变
            if self.content.is_dirty() {
                self.mark_dirty();
            }
        }

        Ok(ret)
    }

    pub async fn remove_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => content.remove_with_key(key, prev_value),
            ObjectMapContent::Hub(content) => content.remove_with_key(cache, key, prev_value).await,
        }?;

        if ret.is_some() {
            assert!(self.total > 0);
            self.total -= 1;

            let size = raw_encode_size(key) + raw_encode_size(ret.as_ref().unwrap());
            assert!(size <= self.size);
            self.size -= size;

            self.deflate_check_point(cache).await?;
            self.mark_dirty();
        }

        debug!(
            "objectmap remove_with_key, key={}, prev={:?}, ret={:?}",
            key, prev_value, ret
        );

        Ok(ret)
    }

    // diff map模式的相关接口
    pub async fn diff_get_by_key(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        match &self.content {
            ObjectMapContent::Simple(content) => content.diff_get_by_key(key),
            ObjectMapContent::Hub(content) => content.diff_get_by_key(cache, key).await,
        }
    }

    pub async fn diff_insert_with_key(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        match &mut self.content {
            ObjectMapContent::Simple(content) => content.diff_insert_with_key(key, value),
            ObjectMapContent::Hub(content) => {
                content
                    .diff_insert_with_key(builder, cache, key, value)
                    .await
            }
        }?;

        self.size += raw_encode_size(key) + raw_encode_size(value);
        self.total += 1;
        self.inflate_check_point(builder, cache).await?;
        self.mark_dirty();

        debug!(
            "objectmap diff_insert_with_key, total={}, {}={}",
            self.total, key, value
        );

        Ok(())
    }

    pub async fn diff_set_with_key(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectMapDiffMapItem,
        prev_value: &Option<ObjectMapDiffMapItem>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => {
                content.diff_set_with_key(key, value, &prev_value, auto_insert)
            }
            ObjectMapContent::Hub(content) => {
                content
                    .diff_set_with_key(builder, cache, key, value, prev_value, auto_insert)
                    .await
            }
        }?;

        debug!(
            "objectmap diff_set_with_key: key={}, value={}, prev={:?}, auto_insert={}, ret={:?}",
            key, value, prev_value, auto_insert, ret
        );

        if ret.is_none() {
            // 插入了一个新元素
            self.size += raw_encode_size(key) + raw_encode_size(value);
            self.total += 1;
            self.inflate_check_point(builder, cache).await?;
            self.mark_dirty();
        } else {
            // 只是发生了replace操作，元素个数不变，大小可能改变
            self.size += raw_encode_size(value);
            self.size -= raw_encode_size(ret.as_ref().unwrap());
            if self.content.is_dirty() {
                self.mark_dirty();
            }
        }

        Ok(ret)
    }

    pub async fn diff_remove_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        prev_value: &Option<ObjectMapDiffMapItem>,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => content.diff_remove_with_key(key, &prev_value),
            ObjectMapContent::Hub(content) => {
                content.diff_remove_with_key(cache, key, prev_value).await
            }
        }?;

        debug!(
            "objectmap diff_remove_with_key, key={}, prev={:?}, ret={:?}",
            key, prev_value, ret
        );

        if ret.is_some() {
            assert!(self.total > 0);
            self.total -= 1;

            let size = raw_encode_size(key) + raw_encode_size(ret.as_ref().unwrap());
            assert!(size <= self.size);
            self.size -= size;

            self.deflate_check_point(cache).await?;
            self.mark_dirty();
        }

        Ok(ret)
    }

    // set模式的相关接口
    pub async fn contains(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        match &self.content {
            ObjectMapContent::Simple(content) => content.contains(object_id),
            ObjectMapContent::Hub(content) => content.contains(cache, object_id).await,
        }
    }

    pub async fn insert(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => content.insert(object_id),
            ObjectMapContent::Hub(content) => content.insert(builder, cache, object_id).await,
        }?;

        if ret {
            self.total += 1;
            self.size += raw_encode_size(object_id);
            self.inflate_check_point(builder, cache).await?;
            self.mark_dirty();
        }

        debug!(
            "objectmap insert, value={}, count={}, ret={}",
            object_id, self.total, ret
        );

        Ok(ret)
    }

    pub async fn remove(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => content.remove(object_id),
            ObjectMapContent::Hub(content) => content.remove(cache, object_id).await,
        }?;

        if ret {
            assert!(self.total > 0);
            self.total -= 1;
            let size = raw_encode_size(object_id);
            assert!(size <= self.size);
            self.size -= size;

            self.deflate_check_point(cache).await?;
            self.mark_dirty();
        }

        debug!(
            "objectmap remove, value={}, count={}, ret={}",
            object_id, self.total, ret
        );

        Ok(ret)
    }

    // diff set模式的相关接口
    pub async fn diff_contains(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        match &self.content {
            ObjectMapContent::Simple(content) => content.diff_contains(object_id),
            ObjectMapContent::Hub(content) => content.diff_contains(cache, object_id).await,
        }
    }

    pub async fn diff_insert(
        &mut self,
        builder: &ObjectMapBuilder,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => content.diff_insert(object_id),
            ObjectMapContent::Hub(content) => content.diff_insert(builder, cache, object_id).await,
        }?;

        if ret {
            self.total += 1;
            self.size += raw_encode_size(object_id);
            self.inflate_check_point(builder, cache).await?;
            self.mark_dirty();
        }

        debug!(
            "objectmap diff_insert, value={}, count={}, ret={}",
            object_id, self.total, ret
        );

        Ok(ret)
    }

    pub async fn diff_remove(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        let ret = match &mut self.content {
            ObjectMapContent::Simple(content) => content.diff_remove(object_id),
            ObjectMapContent::Hub(content) => content.diff_remove(cache, object_id).await,
        }?;

        if ret {
            assert!(self.total > 0);
            self.total -= 1;
            let size = raw_encode_size(object_id);
            assert!(size >= self.size);
            self.size -= size;

            self.deflate_check_point(cache).await?;
            self.mark_dirty();
        }

        debug!(
            "objectmap diff_remove, value={}, count={}, ret={}",
            object_id, self.total, ret
        );

        Ok(ret)
    }

    // visitor
    pub async fn visit(&self, visitor: &mut impl ObjectMapVisitor) -> BuckyResult<()> {
        match &self.content {
            ObjectMapContent::Simple(content) => content.visit(visitor).await,
            ObjectMapContent::Hub(content) => content.visit(visitor).await,
        }
    }
}

impl RawEncode for ObjectMapDescContent {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let ret = u8::raw_bytes().unwrap() // class
            + self.total.raw_measure(purpose)?
            + self.size.raw_measure(purpose)?
            + self.depth.raw_measure(purpose)?
            + self.content_type.raw_measure(purpose)?
            + self.content.raw_measure(purpose)?;

        // for debug
        // debug!("objectmap raw_measure {:?} size={}", self, ret);

        Ok(ret)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let class: u8 = self.class.clone().into();
        let buf = class.raw_encode(buf, purpose)?;

        let buf = self.total.raw_encode(buf, purpose)?;
        let buf = self.size.raw_encode(buf, purpose)?;
        let buf = self.depth.raw_encode(buf, purpose)?;
        let buf = self.content_type.raw_encode(buf, purpose)?;
        let buf = self.content.raw_encode(buf, purpose)?;

        Ok(buf)
    }
}

impl RawDecode<'_> for ObjectMapDescContent {
    fn raw_decode(buf: &[u8]) -> BuckyResult<(Self, &[u8])> {
        let (class, buf) = u8::raw_decode(buf).map_err(|e| {
            error!("ObjectMapDescContent::raw_decode/class error:{}", e);
            e
        })?;

        let class = ObjectMapClass::try_from(class)?;

        let (total, buf) = u64::raw_decode(buf).map_err(|e| {
            error!("ObjectMapDescContent::raw_decode/total error:{}", e);
            e
        })?;

        let (size, buf) = u64::raw_decode(buf).map_err(|e| {
            error!("ObjectMapDescContent::raw_decode/size error:{}", e);
            e
        })?;

        let (depth, buf) = u8::raw_decode(buf).map_err(|e| {
            error!("ObjectMapDescContent::raw_decode/depth error:{}", e);
            e
        })?;

        let (content_type, buf) = ObjectMapSimpleContentType::raw_decode(buf).map_err(|e| {
            error!("ObjectMapDescContent::raw_decode/content_type error:{}", e);
            e
        })?;

        let (content, buf) = ObjectMapContent::raw_decode(buf).map_err(|e| {
            error!("ObjectMapDescContent::raw_decode/content error:{}", e);
            e
        })?;

        // 如果解码后剩余buf不为零，那么hash_value需要重新计算
        let ret = Self {
            class,
            total,
            size,
            depth,
            content_type,
            content,

            hash_cache: ObjectMapContentHashCache::new_empty(),
        };

        Ok((ret, buf))
    }
}

impl DescContent for ObjectMapDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::ObjectMap.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ObjectMapBodyContent;

impl BodyContent for ObjectMapBodyContent {}

pub type ObjectMapType = NamedObjType<ObjectMapDescContent, ObjectMapBodyContent>;
pub type ObjectMapBuilder = NamedObjectBuilder<ObjectMapDescContent, ObjectMapBodyContent>;

pub type ObjectMapDesc = NamedObjectDesc<ObjectMapDescContent>;
pub type ObjectMapId = NamedObjectId<ObjectMapType>;
pub type ObjectMap = NamedObjectBase<ObjectMapType>;

impl ObjectMapBuilder {
    pub fn content_type(&self) -> ObjectMapSimpleContentType {
        self.desc_builder().desc_content().content_type.clone()
    }

    pub fn class(mut self, class: ObjectMapClass) -> Self {
        self.mut_desc_builder().mut_desc_content().class = class;
        self
    }
}

impl ObjectMap {
    pub fn new(
        content_type: ObjectMapSimpleContentType,
        owner: Option<ObjectId>,
        dec_id: Option<ObjectId>,
    ) -> ObjectMapBuilder {
        let desc_content = ObjectMapDescContent::new(ObjectMapClass::Root, content_type, 0);
        let body_content = ObjectMapBodyContent {};

        ObjectMapBuilder::new(desc_content, body_content)
            .option_owner(owner)
            .option_dec_id(dec_id)
            .no_create_time()
    }

    fn new_sub_builder(
        &self,
        content_type: Option<ObjectMapSimpleContentType>,
    ) -> BuckyResult<ObjectMapBuilder> {
        let content_type = match content_type {
            Some(content_type) => content_type,
            None => self.desc().content().content_type().to_owned(),
        };

        // 均衡hash情况下，基本不可能到达这个深度，但我们还是增加一个检测
        let depth = self.depth();
        if depth == u8::MAX {
            let msg = format!("object map depth extend max limit! max={}", u8::MAX);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        let desc_content = ObjectMapDescContent::new(ObjectMapClass::Sub, content_type, depth + 1);
        let body_content = ObjectMapBodyContent {};

        let builder = ObjectMapBuilder::new(desc_content, body_content)
            .option_owner(self.desc().owner().to_owned())
            .option_dec_id(self.desc().dec_id().to_owned())
            .no_create_time();

        Ok(builder)
    }

    pub fn count(&self) -> u64 {
        self.desc().content().count()
    }

    pub fn object_id(&self) -> ObjectId {
        self.flush_id()
    }

    pub fn content_type(&self) -> ObjectMapSimpleContentType {
        self.desc().content().content_type().to_owned()
    }

    pub fn mode(&self) -> ObjectMapContentMode {
        self.desc().content().mode()
    }

    pub fn class(&self) -> ObjectMapClass {
        self.desc().content().class()
    }

    pub fn depth(&self) -> u8 {
        self.desc().content().depth
    }

    pub fn metadata(&self) -> ObjectMapMetaData {
        self.desc().content().metadata()
    }
    // 获取缓存的object_id
    pub fn cached_object_id(&self) -> Option<ObjectId> {
        self.desc().content().hash_cache.object_id()
    }

    // decode后，直接设置id
    pub fn direct_set_object_id_on_init(&self, object_id: &ObjectId) {
        #[cfg(debug_assertions)]
        {
            let real_id = self.flush_id_without_cache();
            assert_eq!(real_id, *object_id);
        }

        self.desc()
            .content()
            .hash_cache
            .direct_set_id_on_init(object_id.to_owned());
    }

    // 刷新并计算id
    pub fn flush_id(&self) -> ObjectId {
        // 首先判断是否需要重新计算id
        if let Some(object_id) = self.desc().content().hash_cache.need_flush_id() {
            return object_id;
        }

        // 发起一次重新计算
        let id = self.desc().calculate_id();

        // 缓存最新的id
        self.desc().content().hash_cache.update_id(&id);

        id
    }

    // 强制发起一次重新计算，不会读取和更新缓存
    pub fn flush_id_without_cache(&self) -> ObjectId {
        self.desc().calculate_id()
    }

    // 模式转换相关函数
    // hub->simple
    pub async fn convert_to_simple(&mut self, cache: &ObjectMapOpEnvCacheRef) -> BuckyResult<()> {
        self.desc_mut().content_mut().convert_to_simple(cache).await
    }

    pub async fn convert_to_hub(&mut self, cache: &ObjectMapOpEnvCacheRef) -> BuckyResult<()> {
        let builder = self.new_sub_builder(None)?;

        self.desc_mut()
            .content_mut()
            .convert_to_hub(&builder, cache)
            .await
    }

    pub fn into_simple(self) -> ObjectMapSimpleContent {
        self.into_desc().into_content().into_simple()
    }

    // 用以对基于path的多级objectmap的支持
    pub async fn get_or_create_child_object_map(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        content_type: ObjectMapSimpleContentType,
        auto_create: ObjectMapCreateStrategy,
    ) -> BuckyResult<Option<ObjectMapRef>> {
        let builder = self.new_sub_builder(Some(content_type))?;
        self.desc_mut()
            .content_mut()
            .get_or_create_child_object_map(&builder, cache, key, auto_create)
            .await
    }

    // list all values/keypairs
    pub async fn list(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        list: &mut ObjectMapContentList,
    ) -> BuckyResult<u64> {
        self.desc().content().list(cache, list).await
    }

    pub async fn list_direct(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
    ) -> BuckyResult<ObjectMapContentList> {
        let mut list = ObjectMapContentList::new(self.count() as usize);
        self.desc().content().list(cache, &mut list).await?;
        Ok(list)
    }

    // list all subs objects in sub mode
    pub async fn list_subs(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        list: &mut Vec<ObjectId>,
    ) -> BuckyResult<u64> {
        self.desc().content().list_subs(cache, list).await
    }

    // 迭代器
    pub async fn next(&self, it: &mut ObjectMapIterator) -> BuckyResult<()> {
        self.desc().content().next(it).await
    }

    // 计算直接的diff，不能同步计算的diff，放到pending列表
    pub(crate) fn diff(&self, other: &Self, diff: &mut ObjectMapDiff) {
        assert_eq!(self.content_type(), other.content_type());

        match self.mode() {
            ObjectMapContentMode::Hub => match other.mode() {
                ObjectMapContentMode::Hub => {
                    self.desc().content().diff(other.desc().content(), diff);
                }
                ObjectMapContentMode::Simple => {
                    diff.pend_async_alter(
                        self.cached_object_id().unwrap(),
                        other.cached_object_id().unwrap(),
                    );
                }
            },
            ObjectMapContentMode::Simple => match other.mode() {
                ObjectMapContentMode::Hub => {
                    diff.pend_async_alter(
                        self.cached_object_id().unwrap(),
                        other.cached_object_id().unwrap(),
                    );
                }
                ObjectMapContentMode::Simple => {
                    self.desc().content().diff(other.desc().content(), diff);
                }
            },
        }
    }

    // map类型相关接口
    pub async fn get_by_key(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
    ) -> BuckyResult<Option<ObjectId>> {
        self.desc().content().get_by_key(cache, key).await
    }

    pub async fn insert_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectId,
    ) -> BuckyResult<()> {
        let builder = self.new_sub_builder(None)?;
        self.desc_mut()
            .content_mut()
            .insert_with_key(&builder, cache, key, value)
            .await
    }

    pub async fn set_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectId,
        prev_value: &Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let builder = self.new_sub_builder(None)?;
        self.desc_mut()
            .content_mut()
            .set_with_key(&builder, cache, key, value, prev_value, auto_insert)
            .await
    }

    pub async fn remove_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        prev_value: &Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        self.desc_mut()
            .content_mut()
            .remove_with_key(cache, key, prev_value)
            .await
    }

    // diffmap类型相关接口
    pub async fn diff_get_by_key(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        self.desc().content().diff_get_by_key(cache, key).await
    }

    pub async fn diff_insert_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        let builder = self.new_sub_builder(None)?;
        self.desc_mut()
            .content_mut()
            .diff_insert_with_key(&builder, cache, key, value)
            .await
    }

    pub async fn diff_set_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        value: &ObjectMapDiffMapItem,
        prev_value: &Option<ObjectMapDiffMapItem>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        let builder = self.new_sub_builder(None)?;
        self.desc_mut()
            .content_mut()
            .diff_set_with_key(&builder, cache, key, value, prev_value, auto_insert)
            .await
    }

    pub async fn diff_remove_with_key(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        key: &str,
        prev_value: &Option<ObjectMapDiffMapItem>,
    ) -> BuckyResult<Option<ObjectMapDiffMapItem>> {
        self.desc_mut()
            .content_mut()
            .diff_remove_with_key(cache, key, prev_value)
            .await
    }

    // set类型相关接口
    pub async fn contains(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        self.desc().content().contains(cache, object_id).await
    }

    pub async fn insert(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        let builder = self.new_sub_builder(None)?;
        self.desc_mut()
            .content_mut()
            .insert(&builder, cache, object_id)
            .await
    }

    pub async fn remove(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        self.desc_mut().content_mut().remove(cache, object_id).await
    }

    // diffset类型相关接口
    pub async fn diff_contains(
        &self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        self.desc().content().diff_contains(cache, object_id).await
    }

    pub async fn diff_insert(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        let builder = self.new_sub_builder(None)?;
        self.desc_mut()
            .content_mut()
            .diff_insert(&builder, cache, object_id)
            .await
    }

    pub async fn diff_remove(
        &mut self,
        cache: &ObjectMapOpEnvCacheRef,
        object_id: &ObjectMapDiffSetItem,
    ) -> BuckyResult<bool> {
        self.desc_mut()
            .content_mut()
            .diff_remove(cache, object_id)
            .await
    }

    pub async fn visit(&self, visitor: &mut impl ObjectMapVisitor) -> BuckyResult<()> {
        self.desc().content().visit(visitor).await
    }
}

#[cfg(test)]
mod test_desc_limit {
    use super::*;

    struct ObjectMapSlim {
        // 对象类别
        class: ObjectMapClass,

        // 子对象个数
        total: u64,

        // 内容总大小
        size: u64,

        // 当前深度，对于根对象，默认是0；对于sub objectmap，那么>0
        depth: u8,

        content_type: ObjectMapSimpleContentType,
    }

    impl RawEncode for ObjectMapSlim {
        fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
            let ret = u8::raw_bytes().unwrap() // class
            + self.total.raw_measure(purpose)?
            + self.size.raw_measure(purpose)?
            + self.depth.raw_measure(purpose)?
            + self.content_type.raw_measure(purpose)?;

            // for debug
            // debug!("objectmap raw_measure {:?} size={}", self, ret);

            Ok(ret)
        }

        fn raw_encode<'a>(
            &self,
            _buf: &'a mut [u8],
            _purpose: &Option<RawEncodePurpose>,
        ) -> BuckyResult<&'a mut [u8]> {
            unimplemented!();
        }
    }

    #[test]
    fn object_map_desc_max_size() {
        let slim = ObjectMapSlim {
            class: ObjectMapClass::Root,
            total: 1024,
            size: 1024,
            depth: 0,
            content_type: ObjectMapSimpleContentType::Map,
        };

        let size = slim.raw_measure(&None).unwrap();
        println!("{}", size);
    }
}

#[cfg(test)]
mod test {
    use super::super::cache::*;
    use super::*;
    use crate::*;

    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    fn gen_random_key(len: usize) -> String {
        let rand_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(len)
            .map(char::from)
            .collect();

        println!("{}", rand_string);
        rand_string
    }

    async fn test_map() {
        let noc = ObjectMapMemoryNOCCache::new();
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        let owner = ObjectId::default();
        let mut map = ObjectMap::new(
            ObjectMapSimpleContentType::Map,
            Some(owner.clone()),
            Some(owner.clone()),
        )
        .no_create_time()
        .build();
        for i in 0..10000 {
            let key = format!("test_map_{}", i);
            let object_id = ObjectId::default();
            map.insert_with_key(&cache, &key, &object_id).await.unwrap();
        }

        let mut subs = vec![];
        map.list_subs(&cache, &mut subs).await.unwrap();
        info!("subs: {:?}", subs);

        let object_id = ObjectId::default();
        for i in 0..10000 {
            let key = format!("test_map_{}", i);
            let ret = map.get_by_key(&cache, &key).await.unwrap();
            assert_eq!(ret, Some(object_id));
        }

        /*
        for i in 0..10000 {
            let key = format!("test_map_{}", i);
            let ret = map.remove_with_key(&cache, &key, &None).await.unwrap();
            assert_eq!(ret, Some(object_id));
        }
        */

        // assert_eq!(map.count(), 0);

        let id = map.flush_id();
        info!("obj map id={}", id);

        cache.put_object_map(&id, map).unwrap();
        cache.gc(true, &id).await.unwrap();
    }

    async fn test_set() {
        let noc = ObjectMapMemoryNOCCache::new();
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        let owner = ObjectId::default();
        let mut map = ObjectMap::new(
            ObjectMapSimpleContentType::Set,
            Some(owner.clone()),
            Some(owner.clone()),
        )
        .no_create_time()
        .build();

        fn object_id_from_index(index: i32) -> ObjectId {
            let key = format!("test_set_{}", index);
            let chunk_id = ChunkId::calculate_sync(key.as_bytes()).unwrap();
            let object_id = chunk_id.object_id();
            object_id
        }

        for i in 0..10000 {
            let object_id = object_id_from_index(i);
            let ret = map.insert(&cache, &object_id).await.unwrap();
            assert!(ret);

            let ret = map.insert(&cache, &object_id).await.unwrap();
            assert!(!ret);
        }

        for i in 0..10000 {
            let object_id = object_id_from_index(i);
            let ret = map.contains(&cache, &object_id).await.unwrap();
            assert!(ret);
        }

        for i in 0..10000 {
            let object_id = object_id_from_index(i);
            let ret = map.remove(&cache, &object_id).await.unwrap();
            assert!(ret);
        }

        assert_eq!(map.count(), 0);

        let id = map.flush_id();
        info!("obj map id={}", id);
    }

    #[test]
    fn test() {
        crate::init_simple_log("test-object-map", Some("debug"));
        async_std::task::block_on(async move {
            // test_set().await;
            test_map().await;
        });
    }

    #[test]
    fn test_path_string() {
        let path = "/a/b/c";
        let parts = path.split("/").skip(1);
        for part in parts {
            println!("part={}", part);
        }
    }

    #[test]
    fn test_hub_fix_limit() {
        let mut content = ObjectMapHubContent {
            depth: 0,
            subs: BTreeMap::new(),
            dirty: false,
        };

        let mut index = 0;
        let object_id = ObjectId::default();
        loop {
            let item = ObjectMapHubItem {
                id: object_id.clone(),
            };
            content.subs.insert(index, item);
            let len = content.raw_measure(&None).unwrap();
            if len > u16::MAX as usize {
                println!("touch desc limit! index ={}", index);
                break;
            }
            index += 1;
        }
    }

    #[test]
    fn test_simple_map_limit() {
        let mut content = ObjectMapSimpleContent::new(ObjectMapSimpleContentType::Map, 0);
        let object_id = ObjectId::default();
        let mut index = 0;
        loop {
            let key = gen_random_key(OBJECT_MAP_KEY_MAX_LEN);
            content.insert_with_key(&key, &object_id).unwrap();
            let len = content.raw_measure(&None).unwrap();
            if len > u16::MAX as usize {
                println!("touch simple map limit! index ={}", index);
                break;
            }

            index += 1;
        }
    }

    #[test]
    fn test_simple_set_limit() {
        use cyfs_base::*;

        let mut content = ObjectMapSimpleContent::new(ObjectMapSimpleContentType::Set, 0);

        let mut index: i32 = 0;
        loop {
            let chunk_id = ChunkId::calculate_sync(&index.to_be_bytes()).unwrap();
            let object_id = chunk_id.object_id();
            let ret = content.insert(&object_id).unwrap();
            assert!(ret);

            let len = content.raw_measure(&None).unwrap();
            if len > u16::MAX as usize {
                println!("touch simple set limit! index ={}", index);
                break;
            }

            index += 1;
        }
    }

    fn hash_bytes(key: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        let mut sha256 = sha2::Sha256::new();
        sha256.input(key);
        hasher.write(&sha256.result());
        hasher.finish()
    }

    #[test]
    fn test_hash() {
        let ret = hash_bytes("abc".as_bytes());
        println!("hash={}", ret);
    }
}
