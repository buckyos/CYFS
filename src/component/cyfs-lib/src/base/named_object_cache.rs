use super::*;
use crate::non::NONPutObjectResult;
use cyfs_base::*;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::convert::{From, TryFrom};
use std::fmt;
use std::sync::Arc;

#[derive(Debug)]
pub enum NamedObjectCacheInsertResult {
    Accept,
    AlreadyExists,
    Updated,
    Merged,
}

impl Into<NONPutObjectResult> for NamedObjectCacheInsertResult {
    fn into(self) -> NONPutObjectResult {
        match self {
            Self::Accept => NONPutObjectResult::Accept,
            Self::AlreadyExists => NONPutObjectResult::AlreadyExists,
            Self::Updated => NONPutObjectResult::Updated,
            Self::Merged => NONPutObjectResult::Merged,
        }
    }
}

impl Into<NamedObjectCacheInsertResult> for NONPutObjectResult {
    fn into(self) -> NamedObjectCacheInsertResult {
        match self {
            Self::Accept | Self::AcceptWithSign => NamedObjectCacheInsertResult::Accept,
            Self::AlreadyExists => NamedObjectCacheInsertResult::AlreadyExists,
            Self::Updated => NamedObjectCacheInsertResult::Updated,
            Self::Merged => NamedObjectCacheInsertResult::Merged,
        }
    }
}

pub struct NamedObjectCacheInsertResponse {
    pub result: NamedObjectCacheInsertResult,
    pub object_update_time: Option<u64>,
    pub object_expires_time: Option<u64>,
}

impl NamedObjectCacheInsertResponse {
    pub fn new(result: NamedObjectCacheInsertResult) -> Self {
        Self {
            result,
            object_update_time: None,
            object_expires_time: None,
        }
    }
    pub fn set_times(&mut self, object: &AnyNamedObject) {
        self.object_expires_time = object.expired_time();
        self.object_update_time = object.update_time();
    }
}

#[derive(Clone)]
pub struct NamedObjectCacheInsertObjectRequest {
    // 来源协议
    pub protocol: NONProtocol,

    // 来源对象
    pub source: DeviceId,

    // 对象id
    pub object_id: ObjectId,

    // 对象所属dec，可以为空
    pub dec_id: Option<ObjectId>,

    // 对象内容
    pub object_raw: Vec<u8>,
    pub object: Arc<AnyNamedObject>,

    // put_flags/get_flags
    pub flags: u32,
}

#[derive(Clone)]
pub struct NamedObjectCacheGetObjectRequest {
    // 来源协议
    pub protocol: NONProtocol,

    // 来源设备
    pub source: DeviceId,

    // 对象id
    pub object_id: ObjectId,
}

#[derive(Clone)]
pub struct NamedObjectCacheDeleteObjectRequest {
    // 来源协议
    pub protocol: NONProtocol,

    // 来源设备
    pub source: DeviceId,

    // 对象id
    pub object_id: ObjectId,

    pub flags: u32,
}

pub struct NamedObjectCacheDeleteObjectResult {
    pub deleted_count: u32,
    pub object: Option<ObjectCacheData>,
}

pub const OBJECT_SELECT_MAX_PAGE_SIZE: u16 = 256;

#[derive(Debug, Clone)]
pub struct NamedObjectCacheSelectObjectOption {
    // 每页读取的数量
    pub page_size: u16,

    // 当前读取的页码，从0开始
    pub page_index: u16,
}

impl Default for NamedObjectCacheSelectObjectOption {
    fn default() -> Self {
        Self {
            page_size: 32_u16,
            page_index: 0_u16,
        }
    }
}

impl TryFrom<&SelectOption> for NamedObjectCacheSelectObjectOption {
    type Error = BuckyError;

    fn try_from(opt: &SelectOption) -> Result<Self, Self::Error> {
        if opt.page_size > OBJECT_SELECT_MAX_PAGE_SIZE {
            let msg = format!(
                "invalid select page_size: {}, max={}",
                opt.page_size, OBJECT_SELECT_MAX_PAGE_SIZE
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        Ok(Self {
            page_size: opt.page_size,
            page_index: opt.page_index,
        })
    }
}

impl From<NamedObjectCacheSelectObjectOption> for SelectOption {
    fn from(opt: NamedObjectCacheSelectObjectOption) -> Self {
        Self {
            page_size: opt.page_size,
            page_index: opt.page_index,
        }
    }
}

// [begin, end)
pub type NamedObjectCacheSelectObjectTimeRange = SelectTimeRange;

#[derive(Debug, Clone)]
pub struct NamedObjectCacheSelectObjectFilter {
    pub obj_type: Option<u16>,
    pub obj_type_code: Option<ObjectTypeCode>,

    pub dec_id: Option<ObjectId>,
    pub owner_id: Option<ObjectId>,
    pub author_id: Option<ObjectId>,

    pub create_time: Option<NamedObjectCacheSelectObjectTimeRange>,
    pub update_time: Option<NamedObjectCacheSelectObjectTimeRange>,
    pub insert_time: Option<NamedObjectCacheSelectObjectTimeRange>,

    // TODO 目前flags只支持全匹配
    pub flags: Option<u32>,
}

impl Default for NamedObjectCacheSelectObjectFilter {
    fn default() -> Self {
        Self {
            obj_type: None,
            obj_type_code: None,
            dec_id: None,
            owner_id: None,
            author_id: None,
            create_time: None,
            update_time: None,
            insert_time: None,

            flags: None,
        }
    }
}

impl From<SelectFilter> for NamedObjectCacheSelectObjectFilter {
    fn from(req: SelectFilter) -> Self {
        Self {
            obj_type: req.obj_type,
            obj_type_code: req.obj_type_code,
            dec_id: req.dec_id,
            owner_id: req.owner_id,
            author_id: req.author_id,
            create_time: req.create_time,
            update_time: req.update_time,
            insert_time: req.insert_time,

            flags: req.flags,
        }
    }
}

impl From<NamedObjectCacheSelectObjectFilter> for SelectFilter {
    fn from(req: NamedObjectCacheSelectObjectFilter) -> Self {
        Self {
            obj_type: req.obj_type,
            obj_type_code: req.obj_type_code,
            dec_id: req.dec_id,
            owner_id: req.owner_id,
            author_id: req.author_id,
            create_time: req.create_time,
            update_time: req.update_time,
            insert_time: req.insert_time,

            flags: req.flags,
        }
    }
}

#[derive(Clone)]
pub struct NamedObjectCacheSelectObjectRequest {
    // 来源协议
    pub protocol: NONProtocol,

    // 来源设备
    pub source: DeviceId,

    // 过滤器
    pub filter: NamedObjectCacheSelectObjectFilter,

    // 可配置选项
    pub opt: Option<NamedObjectCacheSelectObjectOption>,
}

#[derive(Clone)]
pub struct ObjectCacheData {
    // 来源协议
    pub protocol: NONProtocol,

    // 来源对象
    pub source: DeviceId,

    // 对象id
    pub object_id: ObjectId,

    // 对象所属dec，可以为空
    pub dec_id: Option<ObjectId>,

    // 对象内容
    pub object_raw: Option<Vec<u8>>,
    pub object: Option<Arc<AnyNamedObject>>,

    // put_flags/get_flags
    pub flags: u32,

    pub create_time: u64,
    pub update_time: u64,
    pub insert_time: u64,

    // 评级
    pub rank: u8,
}

impl fmt::Display for ObjectCacheData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.protocol.to_string())?;
        write!(f, ",src={}", self.source.to_string())?;
        write!(f, ",obj={}", self.object_id.to_string())?;
        if let Some(dec_id) = &self.dec_id {
            write!(f, ",dec={}", dec_id.to_string())?;
        }
        if let Some(obj) = &self.object {
            write!(f, ",obj_type={}", obj.obj_type())?;
        }
        if let Some(obj) = &self.object_raw {
            write!(f, ",obj_raw_len={}", obj.len())?;
        }
        write!(f, ",flags={}", self.flags)?;
        write!(f, ",create_time={}", self.create_time)?;
        write!(f, ",update_time={}", self.update_time)?;
        write!(f, ",insert_time={}", self.insert_time)?;

        write!(f, ",rank={}", self.rank)?;

        Ok(())
    }
}

impl ObjectCacheData {
    pub fn rebuild_object(&mut self) -> BuckyResult<()> {
        assert!(self.object_raw.is_some());

        if self.object.is_none() {
            let (object, _) = AnyNamedObject::raw_decode(self.object_raw.as_ref().unwrap())?;

            // 校验一次id，有可能缓存了老的数据，但新的协议栈修改了id计算策略，导致不匹配
            // FIXME 这种情况下如何处理？暂时屏蔽此对象不返回了
            let real_object_id = object.calculate_id();
            if self.object_id != real_object_id {
                let msg = format!(
                    "got from noc but unmatch object_id: param object_id={}, calc object_id={}",
                    self.object_id, real_object_id
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }
            self.object = Some(Arc::new(object));
        } else {
            assert_eq!(self.object_id, self.object.as_ref().unwrap().calculate_id());
        }

        #[allow(unused_braces)]
        {
            let obj = self.object.as_ref().unwrap().as_ref();
            self.create_time = Self::get_obj_create_time(obj);

            self.update_time = obj.get_update_time();

            if self.dec_id.is_none() {
                self.dec_id = obj.dec_id().clone();
            }
        }


        Ok(())
    }

    pub fn release_object(&mut self) {
        if let Some(obj) = self.object.take() {
            drop(obj);
        }
    }

    // 更新插入时间
    pub fn update_insert_time(&mut self) {
        self.insert_time = bucky_time_now();
    }

    #[allow(unused_braces)]
    fn get_obj_create_time(obj: &AnyNamedObject) -> u64 {
        let create_time: u64 = match_any_obj!(obj, o, { o.desc().create_time() }, _chunk_id, {
            unreachable!();
        });

        create_time
    }
}

impl From<NamedObjectCacheInsertObjectRequest> for ObjectCacheData {
    fn from(req: NamedObjectCacheInsertObjectRequest) -> Self {
        Self {
            protocol: req.protocol,
            source: req.source,
            object_id: req.object_id,
            dec_id: req.dec_id,
            object_raw: Some(req.object_raw),
            object: Some(req.object),
            flags: req.flags,
            create_time: 0,
            update_time: 0,
            insert_time: 0,
            rank: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedObjectCacheStat {
    pub count: u64,
    pub storage_size: u64,
}

#[async_trait]
pub trait NamedObjectCache: Sync + Send + 'static {
    async fn insert_object(
        &self,
        obj_info: &NamedObjectCacheInsertObjectRequest,
    ) -> BuckyResult<NamedObjectCacheInsertResponse>;

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<ObjectCacheData>>;

    async fn select_object(
        &self,
        req: &NamedObjectCacheSelectObjectRequest,
    ) -> BuckyResult<Vec<ObjectCacheData>>;

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult>;

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat>;

    // sync相关接口
    fn sync_server(&self) -> Option<Box<dyn NamedObjectCacheSyncServer>>;
    fn sync_client(&self) -> Option<Box<dyn NamedObjectCacheSyncClient>>;

    fn clone_noc(&self) -> Box<dyn NamedObjectCache>;
}

pub struct SyncObjectData {
    pub object_id: ObjectId,
    pub seq: u64,
    pub update_time: u64,
}

#[async_trait]
pub trait NamedObjectCacheSyncClient: Sync + Send + 'static {
    // 判断一组对象是否存在，存在的话返回true并更新zone_seq
    async fn diff_objects(&self, list: &Vec<SyncObjectData>) -> BuckyResult<Vec<bool>>;

    // 判断一个要同步的对象存在不存在
    async fn query_object(&self, object_id: &ObjectId, update_time: &u64) -> BuckyResult<bool>;
}

#[async_trait]
pub trait NamedObjectCacheSyncServer: Sync + Send {
    // 获取当前的最新的seq
    async fn get_latest_seq(&self) -> BuckyResult<u64>;

    // 查询指定的同步列表
    async fn list_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        page_index: u16,
        page_size: u16,
    ) -> BuckyResult<Vec<SyncObjectData>>;

    async fn get_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        list: &Vec<ObjectId>,
    ) -> BuckyResult<Vec<ObjectCacheData>>;
}
