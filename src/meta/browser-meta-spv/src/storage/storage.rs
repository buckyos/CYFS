use async_trait::async_trait;
use crate::Config;
use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode};
use crate::storage::sqlite_storage::SqliteStorage;
use cyfs_base_meta::{Block};
use log::*;
use serde::{Serialize};
use crate::storage::mysql_storage::MySqlStorage;

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
pub struct BlockInfo {
    pub height: i64,
    pub id: String,
    pub create_time: u64,
    pub size: u64,
    pub fee: u32,
    pub txs: Vec<String>
}

#[derive(Serialize)]
pub struct TxInfo {
    pub id: String,
    pub create_time: u64,
    pub nonce: i64,
    pub caller: String,
    pub result: u32,
    pub tx_type: u8,
    pub to: String,
    pub fee_used: u64,
    pub block_number: i64,
    pub block_hash: String,
    pub block_create_time: u64
}

#[derive(Serialize)]
pub struct AccountInfo {
    pub txs: u64
}

#[async_trait]
pub trait Storage {
    async fn open(&mut self, config: &Config) -> BuckyResult<()>;
    async fn init(&self) -> BuckyResult<()>;
    // 没有的情况下返回-1
    async fn get_cur_height(&self) -> BuckyResult<i64>;
    async fn get_tx_sum(&self) -> BuckyResult<u64>;

    // 向数据库插入块数据，这里应该把块信息，交易信息等一起插入数据库
    async fn add_block(&self, block: &Block) -> BuckyResult<usize>;

    async fn get_blocks(&self, begin: i64, end: i64, pages: usize, limit: usize) -> BuckyResult<Vec<BlockInfo>>;
    async fn get_txs(&self, begin: i64, end: i64, caller: Option<String>, to: Option<String>, pages: usize, limit: usize) -> BuckyResult<Vec<TxInfo>>;
    async fn get_tx(&self, id: &str) -> BuckyResult<(TxInfo, Vec<u8>, Vec<u8>)>;

    async fn get_account_info(&self, id: &str) -> BuckyResult<AccountInfo>;
}

pub async fn create_storage(config: &Config) -> BuckyResult<Box<dyn Storage + Send + Sync>> {
    match config.engine.as_str() {
        "sqlite" => {
            let mut storage = SqliteStorage::new();
            storage.open(config).await?;
            storage.init().await?;
            Ok(Box::new(storage))
        },
        "mysql" => {
            let mut storage = MySqlStorage::new();
            storage.open(config).await?;
            storage.init().await?;
            Ok(Box::new(storage))
        }
        _ => {
            Err(BuckyError::new(BuckyErrorCode::NotSupport, config.engine.as_str()))
        }
    }
}