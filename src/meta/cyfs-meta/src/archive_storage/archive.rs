use cyfs_base::*;
use async_trait::async_trait;

#[async_trait]
pub trait Archive: Send + Sync {

    async fn init(&self) -> BuckyResult<()>;

    //desc stat
    async fn create_or_update_desc_stat(&self, objid: &ObjectId, obj_type: u8, height: u64) -> BuckyResult<()>;

    // meta raw object
    async fn set_meta_object_stat(
        &self,
        objid: &ObjectId,
        status: u8) -> BuckyResult<()>;

    // meta api stat
    async fn set_meta_api_stat(
        &self,
        api_name: &str,
        status: u8) -> BuckyResult<()>;
}