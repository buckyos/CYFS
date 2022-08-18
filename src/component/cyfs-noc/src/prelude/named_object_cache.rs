use crate::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedObjectStorageCategory {
    Storage = 0,
    Cache = 1,
}

// source device's zone info
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceZoneCategory {
    CurrentDevice,
    CurrentZone,
    FriendsZone,
    OtherZone,
}

#[derive(Clone, Debug)]
pub struct DeviceZoneInfo {
    pub device_id: DeviceId,
    pub zone_category: DeviceZoneCategory,
}

// The identy info of a request
#[derive(Clone, Debug)]
pub struct RequestSourceInfo {
    device: DeviceZoneInfo,
    dec: ObjectId,
}

impl std::fmt::Display for RequestSourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "device=({:?}-{}),dec={}",
            self.device.zone_category, self.device.device_id, self.dec
        )
    }
}

pub struct NamedObjectCachePutObjectRequest {
    pub source: RequestSourceInfo,
    pub object: NONObjectInfo,
    pub storage_category: NamedObjectStorageCategory,
    pub context: Option<String>,
    pub last_access_rpath: Option<String>,
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
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>>;

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse>;

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat1>;
}

pub type NamedObjectCacheRef = Arc<Box<dyn NamedObjectCache1>>;

