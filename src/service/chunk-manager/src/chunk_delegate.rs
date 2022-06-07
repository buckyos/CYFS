use rusqlite::{params};
use cyfs_base::{DeviceId, ChunkId, BuckyResult};
use crate::chunk_meta::{CHUNK_META, get_device_id, get_chunk_id};
use chrono::naive::NaiveTime;

#[derive(Debug)]
pub enum ChunkDelegateState {
    Init = 0,
    Open = 1,
    Close = 2,
    Error = 3,
}

impl ChunkDelegateState{
    pub fn to(&self)->u8{
        match self {
            ChunkDelegateState::Init => 0,
            ChunkDelegateState::Open => 1,
            ChunkDelegateState::Close => 2,
            ChunkDelegateState::Error => 3,
        }
    }

    pub fn from(v: u8)->ChunkDelegateState{
        match v {
            0=> ChunkDelegateState::Init,
            1=> ChunkDelegateState::Open,
            2=> ChunkDelegateState::Close,
            _=> ChunkDelegateState::Error,
        }
    }
}

#[derive(Debug)]
pub struct ChunkDelegate {
    pub id: u32,
    pub miner_device_id: DeviceId,
    pub chunk_id: ChunkId,
    pub state: ChunkDelegateState,
    pub price: i64,
    pub created_at: NaiveTime
}

//
// 委托表: chunk_delegate 
//
const CREATE_CHUNK_DELEGATE_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS "chunk_delegate" (
        "id" INTEGER PRIMARY KEY ASC,
        "miner_device_id" TEXT NOT NULL UNIQUE,
        "chunk_id" TEXT NOT NULL UNIQUE,
        "state" TINYINT(10) NOT NULL DEFAULT 0,
        "price" INTEGER NOT NULL DEFAULT 0,
        "created_at" TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
"#;

const INSERT_CHUNK_DELEGATE_SQL: &str = r#"
    INSERT OR REPLACE INTO chunk_delegate (miner_device_id, chunk_id, price) VALUES (?1, ?2, ?3);
"#;

const UPDATE_CHUNK_DELEGATE_STATE_SQL: &str = r#"
    UPDATE chunk_delegate SET state=?3 where miner_device_id=?1 and chunk_id=?2;
"#;

const SELECT_CHUNK_DELEGATE_PRICE_SQL: &str = r#"
    SELECT price from chunk_delegate WHERE miner_device_id=?1 and chunk_id=?2;
"#;

const SELECT_INIT_CHUNK_DELEGATE_SQL: &str = r#"
    SELECT * from chunk_delegate WHERE state=0 limit ?1;
"#;

const SELECT_CHUNK_DELEGATE_SQL: &str = r#"
    SELECT * from chunk_delegate WHERE chunk_id=?1;
"#;

// 初始化创建表格
pub fn init_table(tables: & mut Vec<String>){
    tables.push(CREATE_CHUNK_DELEGATE_TABLE.to_string());
}

/// 创建chunk委托记录
pub async fn add_chunk_delegate(
    miner_device_id: &DeviceId, 
    chunk_id: &ChunkId, 
    price: &i64,
)-> BuckyResult<()>{
    
    info!("get chunk meta");
    let chunk_meta = CHUNK_META.lock().await;

    info!("do query");
    chunk_meta.query(INSERT_CHUNK_DELEGATE_SQL, params![miner_device_id.to_string(), chunk_id.to_string(), price])
}

pub async fn open_chunk_delegate(
    miner_device_id: &DeviceId, 
    chunk_id: &ChunkId, 
)-> BuckyResult<()>{
    let chunk_meta = CHUNK_META.lock().await;
    chunk_meta.query(UPDATE_CHUNK_DELEGATE_STATE_SQL, 
        params![miner_device_id.to_string(), chunk_id.to_string(), ChunkDelegateState::Open.to()])
}

pub async fn close_chunk_delegate(
    miner_device_id: &DeviceId, 
    chunk_id: &ChunkId, 
)-> BuckyResult<()>{
    let chunk_meta = CHUNK_META.lock().await;
    chunk_meta.query(UPDATE_CHUNK_DELEGATE_STATE_SQL, 
        params![miner_device_id.to_string(), chunk_id.to_string(), ChunkDelegateState::Close.to()])
}

pub async fn fetch_chunk_price(
    miner_device_id: &DeviceId,
    chunk_id: &ChunkId
)->BuckyResult<i64>{
    let chunk_meta = CHUNK_META.lock().await;

    info!("fetch_chunk_price: miner_device_id:{}, chunk_id:{}", miner_device_id.to_string(), chunk_id.to_string());
    chunk_meta.query_row(SELECT_CHUNK_DELEGATE_PRICE_SQL,params![miner_device_id.to_string(), chunk_id.to_string()],|row|->rusqlite::Result<i64>{
        let price = row.get(0)?;
        Ok(price)
    })
}

pub async fn fetch_init_chunk_delegates(limit: i64)->BuckyResult<Vec<ChunkDelegate>>{
    let chunk_meta = CHUNK_META.lock().await;
    let rows = chunk_meta.query_map(SELECT_INIT_CHUNK_DELEGATE_SQL, params![limit],|row|->rusqlite::Result<ChunkDelegate>{
        let p = ChunkDelegate {
            id: row.get(0)?,
            miner_device_id: get_device_id(row, 1)?,
            chunk_id: get_chunk_id(row, 2)?,
            state: ChunkDelegateState::from(row.get(3)?),
            price: row.get(4)?,
            created_at: row.get(5)?,
        };
        Ok(p)
    })?;

    Ok(rows)
}

pub async fn find_delegate(
    chunk_id: &ChunkId
)->BuckyResult<ChunkDelegate>{
    let chunk_meta = CHUNK_META.lock().await;
    chunk_meta.query_row(SELECT_CHUNK_DELEGATE_SQL,
        params![chunk_id.to_string()],|row|{
        let delegate = ChunkDelegate{
            id: row.get(0)?,
            miner_device_id: get_device_id(&row, 1)?,
            chunk_id: get_chunk_id(&row, 2)?,
            state:ChunkDelegateState::from(row.get(3)?),
            price: row.get(4)?,
            created_at: row.get(5)?,
        };

        Ok(delegate)
    })
}