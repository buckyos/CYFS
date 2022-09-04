use crate::meta::*;
use crate::access::*;
use cyfs_base::*;
use cyfs_lib::*;

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

// get_object
#[derive(Clone)]
pub struct NamedObjectCacheGetObjectRequest1 {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,

    pub last_access_rpath: Option<String>,
}

#[derive(Clone)]
pub struct NamedObjectCacheObjectData {
    // object maybe missing while meta info is still here
    pub object: Option<NONObjectInfo>,

    pub meta: NamedObjectMetaData,
}

// delete_object
#[derive(Clone)]
pub struct NamedObjectCacheDeleteObjectRequest1 {
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

// stat
#[derive(Debug, Clone)]
pub struct NamedObjectCacheStat1 {
    pub count: u64,
    pub storage_size: u64,
}

#[async_trait::async_trait]
pub trait NamedObjectCache1: Sync + Send {
    async fn put_object(
        &self,
        req: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse>;

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest1,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>>;

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest1,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse>;

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse>;

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat1>;
}

pub type NamedObjectCacheRef = Arc<Box<dyn NamedObjectCache1>>;