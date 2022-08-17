mod sqlite;

use cyfs_base::*;
use cyfs_lib::NONObjectInfo;

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

    pub update_time: Option<u64>,
    pub expired_time: Option<u64>,

    pub storage_category: NamedObjectStorageCategory,
    pub context: Option<String>,

    pub last_access_rpath: Option<String>,
    pub access_string: u32,
}

impl std::fmt::Display for NamedObjectMetaPutObjectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "source={}, object={}, storage_category={:?}, access={}",
            self.source, self.object_id, self.storage_category, self.access_string
        )?;
        if let Some(owner) = &self.owner_id {
            write!(f, ", owner={}", owner)?;
        }
        if let Some(update_time) = &self.update_time {
            write!(f, ", update_time={}", update_time)?;
        }
        if let Some(expired_time) = &self.expired_time {
            write!(f, ", expired_time={}", expired_time)?;
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
    pub update_time: Option<u64>,
    pub expired_time: Option<u64>,
}

impl std::fmt::Display for NamedObjectMetaPutObjectResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "result={:?}", self.result,)?;

        if let Some(update_time) = &self.update_time {
            write!(f, ", update_time={}", update_time)?;
        }
        if let Some(expired_time) = &self.expired_time {
            write!(f, ", expired_time={}", expired_time)?;
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
}

// delete_object
#[derive(Clone, Debug)]
pub struct NamedObjectMetaDeleteObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,
}

#[derive(Clone, Debug)]
pub struct NamedObjectMetaDeleteObjectResponse {
    pub object: Option<NamedObjectMetaData>,
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

// exists_object
#[derive(Clone, Debug)]
pub struct NamedObjectMetaExistsObjectRequest {
    pub source: RequestSourceInfo,

    pub object_id: ObjectId,
}

#[derive(Debug, Clone)]
pub struct NamedObjectMetaStat {
    pub count: u64,
    pub storage_size: u64,
}

#[async_trait::async_trait]
pub trait NamedObjectMeta: Sync + Send {
    async fn put_object(
        &self,
        request: &NamedObjectMetaPutObjectRequest,
    ) -> BuckyResult<NamedObjectMetaPutObjectResponse>;

    async fn get_object(
        &self,
        request: &NamedObjectMetaGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectMetaData>>;

    async fn delete_object(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<Option<NamedObjectMetaData>>;

    async fn exists_object(&self, req: &NamedObjectMetaExistsObjectRequest) -> BuckyResult<bool>;

    async fn stat(&self) -> BuckyResult<NamedObjectMetaStat>;
}
