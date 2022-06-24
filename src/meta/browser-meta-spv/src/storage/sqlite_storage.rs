use crate::storage::{Storage, map_sql_err, BlockInfo, TxInfo, AccountInfo};
use crate::Config;
use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode, NamedObject, ObjectDesc, RawConvertTo};
use cyfs_base_meta::{Block, BlockTrait, BlockDescTrait};
use sqlx::sqlite::{SqlitePoolOptions, SqliteJournalMode, SqliteConnectOptions, SqliteRow};
use sqlx::{Pool, Sqlite, Transaction, Row, Executor, ConnectOptions};
use std::path::Path;
use std::time::Duration;
use log::*;
use async_trait::async_trait;
use once_cell::sync::OnceCell;
use tide::log::LevelFilter;
use crate::helper::parse_tx;

const INIT_BLOCK_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "Block" (
	"id"	TEXT NOT NULL,
	"height"	INTEGER NOT NULL,
	"create_time"	INTEGER NOT NULL,
	"size"  INTEGER NOT NULL,
	"fee"   INTEGER NOT NULL,
	PRIMARY KEY("id")
)
"#;

const INIT_BLOCK_HEIGHT_INDEX: &str = r#"
CREATE INDEX IF NOT EXISTS "Block_height_index" ON "Block" (
	"height"	DESC
)
"#;

const INIT_TX_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "tx" (
	"id"	TEXT NOT NULL,
	"nonce"	INTEGER NOT NULL,
	"caller"	TEXT NOT NULL,
	"create_time"	INTEGER NOT NULL,
	"type"	INTEGER NOT NULL,
	"to"	TEXT NOT NULL,
	"block_number"	INTEGER NOT NULL,
	PRIMARY KEY("id")
);
"#;

const INIT_TX_HEIGHT_INDEX: &str = r#"
CREATE INDEX IF NOT EXISTS "tx_height_index" ON "tx" (
	"block_number"	DESC
)
"#;

const INIT_TX_RAW_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "tx_raw" (
	"id"	TEXT NOT NULL,
	"raw"	BLOB NOT NULL,
	PRIMARY KEY("id")
)
"#;

const INIT_RECEIPT_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "receipt" (
	"txid"	TEXT NOT NULL,
	"result"	INTEGER NOT NULL,
	"fee_used"	INTEGER NOT NULL,
	PRIMARY KEY("txid")
)
"#;

const INIT_RECEIPT_RAW_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "receipt_raw" (
	"txid"	TEXT NOT NULL,
	"raw"	BLOB NOT NULL,
	PRIMARY KEY("txid")
)
"#;

const INSERT_BLOCK: &str = r#"INSERT into Block VALUES (?1, ?2, ?3, ?4, ?5)"#;
const GET_CUR_HEIGHT: &str = r#"select max(height) as height from Block"#;
const GET_TX_SUM: &str = r#"select count(*) from tx where type != 15"#;

const INSERT_TX: &str = r#"INSERT into tx VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#;
const INSERT_RECEIPT: &str = r#"INSERT into receipt VALUES (?1, ?2, ?3)"#;

const INSERT_TX_RAW: &str = r#"INSERT into tx_raw VALUES (?1, ?2)"#;
const INSERT_RECEIPT_RAW: &str = r#"INSERT into receipt_raw VALUES (?1, ?2)"#;

const GET_BLOCKS: &str = "SELECT * from Block where height >= ?1 AND height <= ?2 ORDER BY height DESC limit ?3 offset ?4";
const GET_TX_IDS: &str = "SELECT id from tx where block_number = ?1 AND type != 15 ";

const GET_TXS: &str = r#"
SELECT t.*, b.id as block_hash, b.create_time as block_create_time, r.result as result, r.fee_used as fee_used
	from tx as t INNER JOIN Block as b on t.block_number = b.height INNER JOIN receipt as r on t.id = r.txid
	where t.block_number >= ?1 AND t.block_number <= ?2 AND t.type != 15 ORDER BY t.ROWID DESC limit ?3 offset ?4
"#;

const GET_TXS_BY_CALLER: &str = r#"
SELECT t.*, b.id as block_hash, b.create_time as block_create_time, r.result as result, r.fee_used as fee_used
	from tx as t INNER JOIN Block as b on t.block_number = b.height INNER JOIN receipt as r on t.id = r.txid
	where t.block_number >= ?1 AND t.block_number <= ?2 AND t.type != 15 AND t.caller = ?3 ORDER BY t.ROWID DESC limit ?4 offset ?5
"#;

const GET_TXS_BY_TO: &str = r#"
SELECT t.*, b.id as block_hash, b.create_time as block_create_time, r.result as result, r.fee_used as fee_used
	from tx as t INNER JOIN Block as b on t.block_number = b.height INNER JOIN receipt as r on t.id = r.txid
	where t.block_number >= ?1 AND t.block_number <= ?2 AND t.type != 15 AND t.to = ?3 ORDER BY t.ROWID DESC limit ?4 offset ?5
"#;

const GET_TX: &str = r#"
SELECT t.*, b.id as block_hash, b.create_time as block_create_time, r.result as result, r.fee_used as fee_used, tr.raw as tx_raw, rw.raw as receipt_raw
	from tx as t INNER JOIN Block as b on t.block_number = b.height INNER JOIN receipt as r on t.id = r.txid
	INNER JOIN tx_raw as tr on t.id = tr.id INNER JOIN receipt_raw as rw on t.id = rw.txid
	where t.id = ?1
"#;

const GET_CALLER_TX_NUM: &str = r#"SELECT count(*) from tx where caller = ?1"#;

pub struct SqliteStorage {
    pool: OnceCell<Pool<Sqlite>>,
}

impl SqliteStorage {
    pub(crate) fn new() -> Self {
        Self {
            pool: OnceCell::new(),
        }
    }
    async fn storage_block(&self, ts: &mut Transaction<'_, Sqlite>, block: &Block) -> BuckyResult<usize> {
        let block_desc = block.desc();
        let block_number = block_desc.number();
        let mut fee = 0;
        let txs = block.transactions();
        let receipts = block.receipts();
        if txs.len() != receipts.len() {
            error!("block {} txs and receipts not match, {} txs and {} receipts", block_desc.hash_str(), txs.len(), receipts.len());
            return Err(BuckyError::from(BuckyErrorCode::NotMatch));
        }
        let mut txs_num = 0;

        for i in 0..txs.len() {
            let tx = &txs[i];
            let receipt = &receipts[i];

            let tx_id = tx.desc().calculate_id().to_string();
            let tx_bodys = tx.desc().content().body.get_obj();
            if tx_bodys.len() == 0 {
                warn!("tx {} not include any txbody", &tx_id);
                continue;
            }

            let (tx_type, to) = parse_tx(&tx_bodys[0]);
            ts.execute(sqlx::query::<Sqlite>(INSERT_TX)
                .bind(&tx_id)
                .bind(tx.desc().content().nonce)
                .bind(tx.desc().content().caller.id()?.to_string())
                .bind(tx.desc().create_time() as i64)
                .bind(tx_type as i8)
                .bind(to)
                .bind(block_number)).await.map_err(map_sql_err)?;
            ts.execute(sqlx::query::<Sqlite>(INSERT_TX_RAW)
                .bind(&tx_id)
                .bind(tx.to_vec()?)).await.map_err(map_sql_err)?;

            ts.execute(sqlx::query::<Sqlite>(INSERT_RECEIPT)
                .bind(&tx_id)
                .bind(receipt.result as i32)
                .bind(receipt.fee_used as i64)).await.map_err(map_sql_err)?;
            ts.execute(sqlx::query::<Sqlite>(INSERT_RECEIPT_RAW)
                .bind(&tx_id)
                .bind(receipt.to_vec()?)).await.map_err(map_sql_err)?;

            fee += receipt.fee_used;

            if tx_type != 15 {
                txs_num += 1;
            }
        }

        let size = block.to_vec().unwrap().len();
        ts.execute(sqlx::query::<Sqlite>(INSERT_BLOCK)
            .bind(block_desc.calculate_id().to_string())
            .bind(block_number)
            .bind(block_desc.create_time() as i64)
            .bind(size as i64)
            .bind(fee as i64)
        ).await.map_err(map_sql_err)?;

        Ok(txs_num)
    }

    async fn get_txids(&self, block_number: i64) -> BuckyResult<Vec<String>> {
        let mut ret = Vec::new();
        let rows = sqlx::query(GET_TX_IDS).bind(block_number)
            .fetch_all(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        for row in rows {
            ret.push(row.get("id"));
        }
        Ok(ret)
    }

    fn txinfo_from_row(&self, row: &SqliteRow) -> TxInfo {
        let create_time: i64 = row.get("create_time");
        let result: i32 = row.get("result");
        let tx_type: i8 = row.get("type");
        let block_create_time: i64 = row.get("block_create_time");
        let fee_used: i64 = row.get("fee_used");
        TxInfo {
            id: row.get("id"),
            create_time: create_time as u64,
            nonce: row.get("nonce"),
            caller: row.get("caller"),
            result: result as u32,
            tx_type: tx_type as u8,
            to: row.get("to"),
            fee_used: fee_used as u64,
            block_number: row.get("block_number"),
            block_hash: row.get("block_hash"),
            block_create_time: block_create_time as u64
        }
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn open(&mut self, config: &Config) -> BuckyResult<()> {
        if let Some(sqlite_config) = &config.sqlite {
            let database = Path::new(sqlite_config.database_path.as_str()).join("database.db");
            let mut options = SqliteConnectOptions::new().filename(database.as_path()).create_if_missing(true)
                .journal_mode(SqliteJournalMode::Memory).busy_timeout(Duration::new(10, 0));
            options.log_statements(LevelFilter::Off);
            let pool = SqlitePoolOptions::new().max_connections(10).connect_with(options).await.map_err(map_sql_err)?;

            let _ = self.pool.set(pool);
            Ok(())
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidFormat, ""))
        }
    }

    async fn init(&self) -> BuckyResult<()> {
        sqlx::query(INIT_BLOCK_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_BLOCK_HEIGHT_INDEX).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_TX_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_TX_HEIGHT_INDEX).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_TX_RAW_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_RECEIPT_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_RECEIPT_RAW_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        Ok(())
    }

    async fn get_cur_height(&self) -> BuckyResult<i64> {
        let row = sqlx::query(GET_CUR_HEIGHT).fetch_one(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        Ok(row.try_get(0).unwrap_or(-1_i64))
    }
    async fn get_tx_sum(&self) -> BuckyResult<u64> {
        let row = sqlx::query(GET_TX_SUM).fetch_one(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        let sum: i64 = row.try_get(0).unwrap_or(0);
        Ok(sum as u64)
    }

    async fn add_block(&self, block: &Block) -> BuckyResult<usize> {
        let mut ts = self.pool.get().unwrap().begin().await.map_err(map_sql_err)?;
        match self.storage_block(&mut ts, block).await {
            Ok(tx_num) => {
                ts.commit().await.map_err(map_sql_err)?;
                Ok(tx_num)
            },
            Err(e) => {
                ts.rollback().await.map_err(map_sql_err)?;
                Err(e)
            }
        }
    }

    async fn get_blocks(&self, begin: i64, end: i64, pages: usize, limit: usize) -> BuckyResult<Vec<BlockInfo>> {
        let rows = sqlx::query(GET_BLOCKS)
            .bind(begin)
            .bind(end)
            .bind(limit as i64)
            .bind((pages*limit) as i64)
            .fetch_all(self.pool.get().unwrap()).await.map_err(map_sql_err)?;

        let mut ret = Vec::new();
        for row in rows {
            let id = row.get("id");
            let height = row.get("height");
            let txs = self.get_txids(height).await?;
            let create_time: i64 = row.get("create_time");
            let size: i64 = row.get("size");
            let fee: i64 = row.get("fee");
            ret.push(BlockInfo {
                height,
                id,
                create_time: create_time as u64,
                size: size as u64,
                fee: fee as u32,
                txs
            });
        }

        Ok(ret)
    }

    async fn get_txs(&self, begin: i64, end: i64, caller: Option<String>, to: Option<String>, pages: usize, limit: usize) -> BuckyResult<Vec<TxInfo>> {
        let rows = if let Some(caller) = caller {
            sqlx::query(GET_TXS_BY_CALLER)
                .bind(begin)
                .bind(end)
                .bind(caller)
        } else {
            if let Some(to) = to {
                sqlx::query(GET_TXS_BY_TO)
                    .bind(begin)
                    .bind(end)
                    .bind(to)
            } else {
                sqlx::query(GET_TXS)
                    .bind(begin)
                    .bind(end)
            }
        }
        .bind(limit as i64)
        .bind((pages*limit) as i64)
        .fetch_all(self.pool.get().unwrap()).await.map_err(map_sql_err)?;

        let mut ret = Vec::new();
        for row in rows {
            ret.push(self.txinfo_from_row(&row));
        }
        Ok(ret)
    }

    async fn get_tx(&self, id: &str) -> BuckyResult<(TxInfo, Vec<u8>, Vec<u8>)> {
        let row = sqlx::query(GET_TX)
            .bind(id)
            .fetch_one(self.pool.get().unwrap()).await.map_err(map_sql_err)?;

        let info = self.txinfo_from_row(&row);
        Ok((info, row.get("tx_raw"), row.get("receipt_raw")))
    }

    async fn get_account_info(&self, id: &str) -> BuckyResult<AccountInfo> {
        let row = sqlx::query(GET_CALLER_TX_NUM).bind(id).fetch_one(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        let sum: i64 = row.try_get(0).unwrap_or(0);
        Ok(AccountInfo {
            txs: sum as u64
        })
    }
}