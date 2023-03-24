use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// put_object
#[derive(Clone, Debug)]
pub enum NamedObjectMetaPutObjectResult {
    Accept,
    AlreadyExists,
    Updated,
}

#[derive(Clone, Debug)]
pub struct NamedObjectMetaPutObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,

    pub owner_id: Option<ObjectId>,

    pub insert_time: u64,
    pub object_type: u16,
    pub object_create_time: Option<u64>,
    pub object_update_time: Option<u64>,
    pub object_expired_time: Option<u64>,
    pub author: Option<ObjectId>,
    pub dec_id: Option<ObjectId>,
    pub prev: Option<ObjectId>,
    pub body_prev_version: Option<HashValue>,
    pub ref_objs: Option<Vec<ObjectLink>>,
    pub nonce: Option<u128>,

    pub storage_category: NamedObjectStorageCategory,
    pub context: Option<String>,

    pub last_access_rpath: Option<String>,
    pub access_string: u32,
}

impl std::fmt::Display for NamedObjectMetaPutObjectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "source={}, object={}, storage_category={:?}, access={}, insert_time={}",
            self.source,
            self.object_id,
            self.storage_category,
            self.access_string,
            self.insert_time,
        )?;
        if let Some(owner) = &self.owner_id {
            write!(f, ", owner={}", owner)?;
        }
        if let Some(update_time) = &self.object_update_time {
            write!(f, ", object_update_time={}", update_time)?;
        }
        if let Some(expired_time) = &self.object_expired_time {
            write!(f, ", object_expired_time={}", expired_time)?;
        }
        if let Some(context) = &self.context {
            write!(f, ", context={}", context)?;
        }
        if let Some(last_access_rpath) = &self.last_access_rpath {
            write!(f, ", last_access_rpath={}", last_access_rpath)?;
        }

        Ok(())
    }
}

pub struct NamedObjectMetaPutObjectResponse {
    pub result: NamedObjectMetaPutObjectResult,
    pub object_update_time: Option<u64>,
    pub object_expired_time: Option<u64>,
}

impl std::fmt::Display for NamedObjectMetaPutObjectResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "result={:?}", self.result,)?;

        if let Some(update_time) = &self.object_update_time {
            write!(f, ", object_update_time={}", update_time)?;
        }
        if let Some(expired_time) = &self.object_expired_time {
            write!(f, ", object_expired_time={}", expired_time)?;
        }

        Ok(())
    }
}

// get_object
#[derive(Clone, Debug)]
pub struct NamedObjectMetaGetObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,

    pub last_access_rpath: Option<String>,

    pub flags: u32,
}

impl NamedObjectMetaGetObjectRequest {
    pub fn is_no_update_last_access(&self) -> bool {
        self.flags & NAMED_OBJECT_CACHE_GET_OBJECT_FLAG_NO_UPDATE_LAST_ACCESS == NAMED_OBJECT_CACHE_GET_OBJECT_FLAG_NO_UPDATE_LAST_ACCESS
    }
}

// update_last_access
#[derive(Clone, Debug)]
pub struct NamedObjectMetaUpdateLastAccessRequest {
    // Shoule verify the access before call update_last_access!
    // pub source: RequestSourceInfo,
    pub object_id: ObjectId,
    pub last_access_time: u64,
    pub last_access_rpath: Option<String>,
}

// delete_object
#[derive(Clone, Debug)]
pub struct NamedObjectMetaDeleteObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,

    pub flags: u32,
}

#[derive(Clone, Debug)]
pub struct NamedObjectMetaDeleteObjectResponse {
    pub deleted_count: u32,
    pub object: Option<NamedObjectMetaData>,
}

// exists_object
#[derive(Clone, Debug)]
pub struct NamedObjectMetaExistsObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,
}

pub type NamedObjectMetaUpdateObjectMetaRequest = NamedObjectCacheUpdateObjectMetaRequest;
pub type NamedObjectMetaCheckObjectAccessRequest = NamedObjectCacheCheckObjectAccessRequest;

#[derive(Debug, Clone)]
pub struct NamedObjectMetaStat {
    pub count: u64,
    pub storage_size: u64,
}

pub type NamedObjectMetaSelectObjectRequest = NamedObjectCacheSelectObjectRequest;
pub type NamedObjectMetaSelectObjectResponse = NamedObjectCacheSelectObjectResponse;

#[async_trait::async_trait]
pub trait NamedObjectMeta: Sync + Send {
    async fn put_object(
        &self,
        req: &NamedObjectMetaPutObjectRequest,
    ) -> BuckyResult<NamedObjectMetaPutObjectResponse>;

    async fn get_object(
        &self,
        req: &NamedObjectMetaGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectMetaData>>;

    async fn delete_object(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse>;

    async fn exists_object(&self, req: &NamedObjectMetaExistsObjectRequest) -> BuckyResult<bool>;

    async fn update_last_access(
        &self,
        req: &NamedObjectMetaUpdateLastAccessRequest,
    ) -> BuckyResult<bool>;

    async fn update_object_meta(
        &self,
        req: &NamedObjectMetaUpdateObjectMetaRequest,
    ) -> BuckyResult<()>;

    async fn check_object_access(
        &self,
        req: &NamedObjectMetaCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>>;

    async fn stat(&self) -> BuckyResult<NamedObjectMetaStat>;

    async fn select_object(
        &self,
        req: &NamedObjectMetaSelectObjectRequest,
    ) -> BuckyResult<NamedObjectMetaSelectObjectResponse>;

    fn bind_object_meta_access_provider(
        &self,
        object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    );
}

pub type NamedObjectMetaRef = Arc<Box<dyn NamedObjectMeta>>;
