use cyfs_base::*;
use async_trait::async_trait;

#[async_trait]
pub trait Archive: Send + Sync {

    async fn being_transaction(&self) -> BuckyResult<()>;
    async fn rollback(&self) -> BuckyResult<()>;
    async fn commit(&self) -> BuckyResult<()>;

    async fn init(&self) -> BuckyResult<()>;

    //desc stat
    async fn create_obj_desc_stat(&self, objid: &ObjectId, obj_type: u8) -> BuckyResult<()>;
    
    async fn get_obj_desc_stat(&self, obj_type: u8) -> BuckyResult<u64>;

    async fn get_daily_added_desc(
        &self,
        obj_type: u8,
        date: u64) -> BuckyResult<u64>;

    async fn get_daily_active_desc(
        &self,
        obj_type: u8,
        date: u64) -> BuckyResult<u64>;

    async fn update_obj_desc_stat(
        &self,
        objid: &ObjectId,
        obj_type:u8) -> BuckyResult<()>;
    async fn drop_desc_stat(&self, obj_id: &ObjectId) -> BuckyResult<()>;

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