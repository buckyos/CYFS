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

// get_object
#[derive(Clone)]
pub struct NamedObjectCacheGetObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,

    pub last_access_rpath: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NamedObjectMetaData {
    pub object_id: ObjectId,

    pub owner_id: Option<ObjectId>,
    pub create_dec_id: ObjectId,

    pub update_time: Option<u64>,
    pub expired_time: Option<u64>,

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

// stat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedObjectCacheStat {
    pub count: u64,
    pub storage_size: u64,
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

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat>;
}

pub type NamedObjectCacheRef = Arc<Box<dyn NamedObjectCache>>;
