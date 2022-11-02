use async_trait::async_trait;
use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode};
use crate::sqlite_storage::SqliteStorage;
use log::*;
use serde::{Serialize};

pub fn map_sql_err(e: sqlx::Error) -> BuckyError {
    match e {
        sqlx::Error::RowNotFound => {
            BuckyError::from(BuckyErrorCode::NotFound)
        }
        _ => {
            let msg = format!("sql error: {:?}", e);
            error!("{}", &msg);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        }
    }
}

#[derive(Serialize)]
pub struct MetaStat {
    pub id: String,
    pub success: u64,
    pub failed: u64,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Period {
    Daily = 0,
    Weekly = 1,
    Month = 2,
}

#[async_trait]
pub trait Storage {
    async fn open(&mut self, db_path: &str) -> BuckyResult<()>;

    async fn init(&self) -> BuckyResult<()>;
    // people/device 数目
    async fn get_desc(&self, obj_type: u8) -> BuckyResult<u64>;
    // people/device 新增
    async fn get_desc_add(&self, obj_type: u8, period: Period) -> BuckyResult<u64>;
    // people/device 活跃
    async fn get_desc_active(&self, obj_type: u8, period: Period) -> BuckyResult<u64>;

    // meta api success/failed
    async fn get_meta_api_stat(&self) -> BuckyResult<Vec<MetaStat>>;

    // meta object success/failed
    async fn get_meta_object_stat(&self) -> BuckyResult<Vec<MetaStat>>;

}

pub async fn create_storage(db_path: &str) -> BuckyResult<Box<dyn Storage + Send + Sync>> {
    let mut storage = SqliteStorage::new();
    storage.open(db_path).await?;
    storage.init().await?;
    Ok(Box::new(storage))
}