use crate::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Whether the delete operation returns the original value, the default does not return
pub const CYFS_NOC_FLAG_DELETE_WITH_QUERY: u32 = 0x01 << 1;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedObjectStorageCategory {
    Storage = 0,
    Cache = 1,
}

impl NamedObjectStorageCategory {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl Default for NamedObjectStorageCategory {
    fn default() -> Self {
        Self::Cache
    }
}

impl TryFrom<u8> for NamedObjectStorageCategory {
    type Error = BuckyError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let v = match value {
            0 => Self::Storage,
            1 => Self::Cache,
            _ => {
                let msg = format!("invalid NamedObjectStorageCategory value: {}", value);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        };

        Ok(v)
    }
}

pub struct NamedObjectCachePutObjectRequest {
    pub source: RequestSourceInfo,
    pub object: NONObjectInfo,
    pub storage_category: NamedObjectStorageCategory,
    pub context: Option<String>,
    pub last_access_rpath: Option<String>,
    pub access_string: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub enum NamedObjectCachePutObjectResult {
    Accept,
    AlreadyExists,
    Updated,
    Merged,
}

pub struct NamedObjectCachePutObjectResponse {
    pub result: NamedObjectCachePutObjectResult,
    pub update_time: Option<u64>,
    pub expires_time: Option<u64>,
}

// update object's related meta info (except meta info from object data)
#[derive(Debug)]
pub struct NamedObjectCacheUpdateObjectMetaRequest {
    pub source: RequestSourceInfo,
    pub object_id: ObjectId,

    pub storage_category: Option<NamedObjectStorageCategory>,
    pub context: Option<String>,
    pub last_access_rpath: Option<String>,
    pub access_string: Option<u32>,
}

impl NamedObjectCacheUpdateObjectMetaRequest {
    pub fn is_empty(&self) -> bool {
        self.storage_category.is_none()
            && self.context.is_none()
            && self.last_access_rpath.is_none()
            && self.access_string.is_none()
    }
}

pub const NAMED_OBJECT_CACHE_GET_OBJECT_FLAG_NO_UPDATE_LAST_ACCESS: u32 = 0x01;

// get_object
#[derive(Clone)]
pub struct NamedObjectCacheGetObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,

    pub last_access_rpath: Option<String>,

    pub flags: u32,
}

impl NamedObjectCacheGetObjectRequest {
    pub fn set_no_update_last_access(&mut self) {
        self.flags |= NAMED_OBJECT_CACHE_GET_OBJECT_FLAG_NO_UPDATE_LAST_ACCESS;
    }
    
    pub fn is_no_update_last_access(&self) -> bool {
        self.flags & NAMED_OBJECT_CACHE_GET_OBJECT_FLAG_NO_UPDATE_LAST_ACCESS == NAMED_OBJECT_CACHE_GET_OBJECT_FLAG_NO_UPDATE_LAST_ACCESS
    }
}

#[derive(Clone, Debug)]
pub struct NamedObjectMetaData {
    pub object_id: ObjectId,
    pub object_type: u16,

    pub owner_id: Option<ObjectId>,
    pub create_dec_id: ObjectId,

    // the item in noc's related times
    pub insert_time: u64,
    pub update_time: u64,

    // object's create_time, update_time and expired_time
    pub object_create_time: Option<u64>,
    pub object_update_time: Option<u64>,
    pub object_expired_time: Option<u64>,

    // object related fields
    pub author: Option<ObjectId>,
    pub dec_id: Option<ObjectId>,

    pub storage_category: NamedObjectStorageCategory,
    pub context: Option<String>,

    pub last_access_rpath: Option<String>,
    pub access_string: u32,
}

#[derive(Clone)]
pub struct NamedObjectCacheObjectRawData {
    // object maybe missing while meta info is still here
    pub object: Option<NONObjectInfo>,

    pub meta: NamedObjectMetaData,
}

#[derive(Clone)]
pub struct NamedObjectCacheObjectData {
    // object must be there
    pub object: NONObjectInfo,

    pub meta: NamedObjectMetaData,
}

// delete_object
#[derive(Clone)]
pub struct NamedObjectCacheDeleteObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,
    pub flags: u32,
}

#[derive(Clone)]
pub struct NamedObjectCacheDeleteObjectResponse {
    pub deleted_count: u32,

    // object maybe missing while meta info is still here
    pub object: Option<NONObjectInfo>,

    pub meta: Option<NamedObjectMetaData>,
}

// exists_object
pub struct NamedObjectCacheExistsObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,
}

pub struct NamedObjectCacheExistsObjectResponse {
    pub meta: bool,
    pub object: bool,
}

// check_access
pub struct NamedObjectCacheCheckObjectAccessRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,
    pub required_access: AccessPermissions,
}

// stat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedObjectCacheStat {
    pub count: u64,
    pub storage_size: u64,
}

#[derive(Debug, Clone)]
pub struct NamedObjectCacheSelectObjectFilter {
    pub obj_type: Option<u16>,
}

impl Default for NamedObjectCacheSelectObjectFilter {
    fn default() -> Self {
        Self {
            obj_type: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NamedObjectCacheSelectObjectOption {
    // The number of readings per page
    pub page_size: usize,

    // The page number currently read, starting from 0
    pub page_index: usize,
}

impl Default for NamedObjectCacheSelectObjectOption {
    fn default() -> Self {
        Self {
            page_size: 256,
            page_index: 0,
        }
    }
}

#[derive(Clone)]
pub struct NamedObjectCacheSelectObjectRequest {
    // filters
    pub filter: NamedObjectCacheSelectObjectFilter,

    // configs
    pub opt: NamedObjectCacheSelectObjectOption,
}

#[derive(Debug)]
pub struct NamedObjectCacheSelectObjectData {
    pub object_id: ObjectId,
}

#[derive(Debug)]
pub struct NamedObjectCacheSelectObjectResponse {
    pub list: Vec<NamedObjectCacheSelectObjectData>,
}

#[async_trait::async_trait]
pub trait NamedObjectCache: Sync + Send {
    async fn put_object(
        &self,
        req: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse>;

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>> {
        match self.get_object_raw(req).await? {
            Some(ret) => match ret.object {
                Some(object) => Ok(Some(NamedObjectCacheObjectData {
                    object,
                    meta: ret.meta,
                })),
                None => {
                    warn!(
                        "get object meta from noc but object missing! {}",
                        req.object_id
                    );
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    async fn get_object_raw(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>>;

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse>;

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse>;

    async fn update_object_meta(
        &self,
        req: &NamedObjectCacheUpdateObjectMetaRequest,
    ) -> BuckyResult<()>;

    async fn check_object_access(
        &self,
        req: &NamedObjectCacheCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>>;

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat>;

    // for internal use only
    async fn select_object(
        &self,
        req: &NamedObjectCacheSelectObjectRequest,
    ) -> BuckyResult<NamedObjectCacheSelectObjectResponse>;

    fn bind_object_meta_access_provider(
        &self,
        object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    );
}

pub type NamedObjectCacheRef = Arc<Box<dyn NamedObjectCache>>;

impl ObjectSelectorDataProvider for NamedObjectMetaData {
    fn object_id(&self) -> &ObjectId {
        &self.object_id
    }
    fn obj_type(&self) -> u16 {
        self.object_type
    }

    fn object_dec_id(&self) -> &Option<ObjectId> {
        &self.dec_id
    }
    fn object_author(&self) -> &Option<ObjectId> {
        &self.author
    }
    fn object_owner(&self) -> &Option<ObjectId> {
        &self.owner_id
    }

    fn object_create_time(&self) -> Option<u64> {
        self.object_create_time
    }
    fn object_update_time(&self) -> Option<u64> {
        self.object_update_time
    }
    fn object_expired_time(&self) -> Option<u64> {
        self.object_expired_time
    }

    fn update_time(&self) -> &u64 {
        &self.update_time
    }
    fn insert_time(&self) -> &u64 {
        &self.insert_time
    }
}

#[async_trait::async_trait]
pub trait NamedObjectCacheObjectMetaAccessProvider: Sync + Send {
    async fn check_access(
        &self,
        target_dec_id: &ObjectId,
        object_data: &dyn ObjectSelectorDataProvider,
        source: &RequestSourceInfo,
        permissions: AccessPermissions,
    ) -> BuckyResult<Option<()>>;
}

pub type NamedObjectCacheObjectMetaAccessProviderRef = Arc<Box<dyn NamedObjectCacheObjectMetaAccessProvider>>;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum NamedObjectRelationType {
    InnerPath = 0,
}

impl Into<u8> for NamedObjectRelationType {
    fn into(self) -> u8 {
        match self {
            Self::InnerPath => 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NamedObjectRelationCacheKey {
    pub object_id: ObjectId,
    pub relation_type: NamedObjectRelationType,
    pub relation: String,
}

#[derive(Clone, Debug)]
pub struct NamedObjectRelationCachePutRequest {
    pub cache_key: NamedObjectRelationCacheKey,
    pub target_object_id: Option<ObjectId>,
}

#[derive(Clone, Debug)]
pub struct NamedObjectRelationCacheGetRequest {
    pub cache_key: NamedObjectRelationCacheKey,
    pub flags: u32,
}

#[derive(Clone)]
pub struct NamedObjectRelationCacheData {
    pub target_object_id: Option<ObjectId>,
}

#[async_trait::async_trait]
pub trait NamedObjectRelationCache: Send + Sync {
    async fn put(&self, req: &NamedObjectRelationCachePutRequest) -> BuckyResult<()>;
    async fn get(&self, req: &NamedObjectRelationCacheGetRequest) -> BuckyResult<Option<NamedObjectRelationCacheData>>;
}

pub type NamedObjectRelationCacheRef = Arc<Box<dyn NamedObjectRelationCache>>;
