use crate::chunk_manager;
use crate::chunk_meta::CHUNK_META;
use cyfs_base::*;
use cyfs_base_meta::*;
use rusqlite::{params};
use chrono::naive::NaiveTime;

pub enum ChunkTxState {
    Init = 0,        // 初始化
    Sign = 1,        // 签名
    DoubleSign = 2, // 双签名
    Chain = 3,       // 上链
}

fn state_to_i16(state: ChunkTxState)->i16{
    match state {
        ChunkTxState::Init=>0,
        ChunkTxState::Sign=>1,
        ChunkTxState::DoubleSign=>2,
        ChunkTxState::Chain=>3,
    }
}

fn i16_to_state(state: i32 )->ChunkTxState{
    match state {
        0=>ChunkTxState::Init,
        1=>ChunkTxState::Sign,
        2=>ChunkTxState::DoubleSign,
        3=>ChunkTxState::Chain,
        _=>ChunkTxState::Init,
    }
}

pub struct ChunkTx {
    pub seq: i64,
    pub source_device_id: DeviceId,
    pub miner_device_id: DeviceId,
    pub client_device_id: DeviceId,
    pub chunk_id: ChunkId,
    pub state: ChunkTxState,
    pub tx: String,
    pub created_at: NaiveTime,
}

//
// 委托表: chunk_delegate
//
const CREATE_CHUNK_TX_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS "chunk_tx" (
        "seq" INTEGER PRIMARY KEY AUTOINCREMENT,
        "source_device_id" TEXT NOT NULL,
        "miner_device_id" TEXT NOT NULL,
        "client_device_id" TEXT NOT NULL,
        "chunk_id" TEXT NOT NULL,
        "value" INTEGER NOT NULL DEFAULT 0,
        "state" TINYINT(10) NOT NULL DEFAULT 0,
        "tx" TEXT NOT NULL DEFAULT "",
        "created_at" TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
"#;

const INSERT_CHUNK_TX_SQL: &str = r#"
    INSERT OR REPLACE INTO chunk_tx (source_device_id, miner_device_id, client_device_id, chunk_id) VALUES (?1, ?2, ?3, ?4);
"#;

const SELECT_DEVIATION_BY_SEQ_SQL: &str = r#"
    SELECT MAX(value)+?2 FROM chunk_tx where miner_device_id=?1;
"#;

const FILL_CHUNK_TX_BY_SEQ_SQL: &str = r#"
    UPDATE chunk_tx SET value=?2, state=?3, tx=?4 where seq=?1;
"#;

// 初始化创建表格
pub fn init_table(tables: & mut Vec<String>){
    tables.push(CREATE_CHUNK_TX_TABLE.to_string());
}

pub async fn add_chunk_tx(
    chunk_manager: &chunk_manager::ChunkManager,
    miner_device_id: &DeviceId,
    client_device_id: &DeviceId,
    chunk_id: &ChunkId,
    price: i64,
)-> BuckyResult<DeviateUnionTx>{
    let source_device_id = chunk_manager.get_device_id();

    let chunk_meta = CHUNK_META.lock().await;

    // insert chunk tx
    let seq = chunk_meta.insert(INSERT_CHUNK_TX_SQL, params![
        source_device_id.to_string(),
        miner_device_id.to_string(),
        client_device_id.to_string(),
        chunk_id.to_string(),
    ])?;

    // calculate current deviation
    let value = chunk_meta.query_row(SELECT_DEVIATION_BY_SEQ_SQL,
        params![
            miner_device_id.to_string(),
            price
        ],
        |row|->rusqlite::Result<i64>{
            let value = row.get(0)?;
            Ok(value)
        }
    )?;
    let deviation = -value;

    // sign tx
    let sign_tx = chunk_manager.sign_trans(miner_device_id, seq, deviation).await?;
    let sign_tx_str = sign_tx.to_hex()?;

    // update chunk tx
    let state = state_to_i16(ChunkTxState::Sign);
    chunk_meta.query(FILL_CHUNK_TX_BY_SEQ_SQL, params![seq, value, state, sign_tx_str])?;

    Ok(sign_tx)
}
