use crate::storage::{Storage, BlockInfo, TxInfo, AccountInfo, map_sql_err};
use crate::Config;
use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode, NamedObject, ObjectDesc, RawConvertTo};
use cyfs_base_meta::{Block, BlockDescTrait, BlockTrait};
use once_cell::sync::OnceCell;
use sqlx::{Pool, MySql, ConnectOptions, Row, Transaction, Executor};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlRow};
use log::LevelFilter;
use log::*;
use crate::helper::parse_tx;
use async_trait::async_trait;

const INIT_BLOCK_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS `block` (
  `id` VARCHAR(64) NOT NULL,
  `height` BIGINT NOT NULL,
  `create_time` BIGINT UNSIGNED NOT NULL,
  `size` BIGINT UNSIGNED NOT NULL,
  `fee` BIGINT UNSIGNED NOT NULL,
  PRIMARY KEY (`id`),
  UNIQUE INDEX `id_UNIQUE` (`id` ASC) VISIBLE,
  INDEX `height` (`height` DESC) VISIBLE);
"#;

const INIT_RECEIPT_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS `receipt` (
  `txid` VARCHAR(64) NOT NULL,
  `result` INT UNSIGNED NOT NULL,
  `fee_used` BIGINT UNSIGNED NOT NULL,
  PRIMARY KEY (`txid`),
  UNIQUE INDEX `txid_UNIQUE` (`txid` ASC) VISIBLE);
"#;

const INIT_RECEIPT_RAW_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS `receipt_raw` (
  `txid` VARCHAR(64) NOT NULL,
  `raw` BLOB NOT NULL,
  PRIMARY KEY (`txid`),
  UNIQUE INDEX `txid_UNIQUE` (`txid` ASC) VISIBLE);
"#;

const INIT_TX_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS `tx` (
  `id` varchar(64) NOT NULL,
  `nonce` bigint NOT NULL,
  `caller` varchar(64) NOT NULL,
  `create_time` bigint unsigned NOT NULL,
  `type` int unsigned NOT NULL,
  `to` varchar(64) DEFAULT NULL,
  `block_number` bigint NOT NULL,
  `rowid` bigint unsigned NOT NULL AUTO_INCREMENT,
  PRIMARY KEY (`id`),
  UNIQUE INDEX `id_UNIQUE` (`id`),
  UNIQUE INDEX `rowid_UNIQUE` (`rowid` DESC),
  INDEX `height` (`block_number` DESC)
)
"#;

const INIT_TX_RAW_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS `tx_raw` (
  `id` VARCHAR(64) NOT NULL,
  `raw` LONGBLOB NOT NULL,
  PRIMARY KEY (`id`),
  UNIQUE INDEX `id_UNIQUE` (`id` ASC) VISIBLE);
"#;

const INSERT_BLOCK: &str = r#"INSERT into block VALUES (?, ?, ?, ?, ?)"#;
const GET_CUR_HEIGHT: &str = r#"select max(height) as height from block"#;
const GET_TX_SUM: &str = r#"select count(*) from tx where type != 15"#;

const INSERT_TX: &str = r#"
INSERT into tx (`id`, `nonce`, `caller`, `create_time`, `type`, `to`, `block_number`) VALUES (?, ?, ?, ?, ?, ?, ?)
"#;
const INSERT_RECEIPT: &str = r#"INSERT into receipt VALUES (?, ?, ?)"#;

const INSERT_TX_RAW: &str = r#"INSERT into tx_raw VALUES (?, ?)"#;
const INSERT_RECEIPT_RAW: &str = r#"INSERT into receipt_raw VALUES (?, ?)"#;

const GET_BLOCKS: &str = "SELECT * from block where height >= ? AND height <= ? ORDER BY height DESC limit ? offset ?";
const GET_TX_IDS: &str = "SELECT id from tx where block_number = ? AND type != 15 ";

const GET_TXS: &str = r#"
SELECT t.*, b.id as block_hash, b.create_time as block_create_time, r.result as result, r.fee_used as fee_used
	from tx as t INNER JOIN block as b on t.block_number = b.height INNER JOIN receipt as r on t.id = r.txid
	where t.block_number >= ? AND t.block_number <= ? AND t.type != 15 ORDER BY t.rowid DESC limit ? offset ?
"#;

const GET_TXS_BY_CALLER: &str = r#"
SELECT t.*, b.id as block_hash, b.create_time as block_create_time, r.result as result, r.fee_used as fee_used
	from tx as t INNER JOIN block as b on t.block_number = b.height INNER JOIN receipt as r on t.id = r.txid
	where t.block_number >= ? AND t.block_number <= ? AND t.type != 15 AND t.caller = ? ORDER BY t.rowid DESC limit ? offset ?
"#;

const GET_TXS_BY_TO: &str = r#"
SELECT t.*, b.id as block_hash, b.create_time as block_create_time, r.result as result, r.fee_used as fee_used
	from tx as t INNER JOIN block as b on t.block_number = b.height INNER JOIN receipt as r on t.id = r.txid
	where t.block_number >= ? AND t.block_number <= ? AND t.type != 15 AND t.to = ? ORDER BY t.rowid DESC limit ? offset ?
"#;

const GET_TX: &str = r#"
SELECT t.*, b.id as block_hash, b.create_time as block_create_time, r.result as result, r.fee_used as fee_used, tr.raw as tx_raw, rw.raw as receipt_raw
	from tx as t INNER JOIN block as b on t.block_number = b.height INNER JOIN receipt as r on t.id = r.txid
	INNER JOIN tx_raw as tr on t.id = tr.id INNER JOIN receipt_raw as rw on t.id = rw.txid
	where t.id = ?
"#;

const GET_CALLER_TX_NUM: &str = r#"SELECT count(*) from tx where caller = ?"#;

pub struct MySqlStorage {
    pool: OnceCell<Pool<MySql>>,
}

impl MySqlStorage {
    pub(crate) fn new() -> Self {
        Self {
            pool: OnceCell::new(),
        }
    }

    async fn storage_block(&self, ts: &mut Transaction<'_, MySql>, block: &Block) -> BuckyResult<usize> {
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
            info!("insert tx");
            ts.execute(sqlx::query(INSERT_TX)
                .bind(&tx_id)
                .bind(tx.desc().content().nonce)
                .bind(tx.desc().content().caller.id()?.to_string())
                .bind(tx.desc().create_time())
                .bind(tx_type)
                .bind(to)
                .bind(block_number)).await.map_err(map_sql_err)?;
            info!("insert tx raw");
            ts.execute(sqlx::query(INSERT_TX_RAW)
                .bind(&tx_id)
                .bind(tx.to_vec()?)).await.map_err(map_sql_err)?;

            info!("insert receipt");
            ts.execute(sqlx::query(INSERT_RECEIPT)
                .bind(&tx_id)
                .bind(receipt.result)
                .bind(receipt.fee_used)).await.map_err(map_sql_err)?;
            info!("insert receipt raw");
            ts.execute(sqlx::query(INSERT_RECEIPT_RAW)
                .bind(&tx_id)
                .bind(receipt.to_vec()?)).await.map_err(map_sql_err)?;

            fee += receipt.fee_used;

            if tx_type != 15 {
                txs_num += 1;
            }
        }

        let size = block.to_vec().unwrap().len();
        ts.execute(sqlx::query(INSERT_BLOCK)
            .bind(block_desc.calculate_id().to_string())
            .bind(block_number)
            .bind(block_desc.create_time())
            .bind(size as u32)
            .bind(fee)
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

    fn txinfo_from_row(&self, row: &MySqlRow) -> TxInfo {
        TxInfo {
            id: row.get("id"),
            create_time: row.get("create_time"),
            nonce: row.get("nonce"),
            caller: row.get("caller"),
            result: row.get("result"),
            tx_type: row.get("type"),
            to: row.get("to"),
            fee_used: row.get("fee_used"),
            block_number: row.get("block_number"),
            block_hash: row.get("block_hash"),
            block_create_time: row.get("block_create_time")
        }
    }
}

#[async_trait]
impl Storage for MySqlStorage {
    async fn open(&mut self, config: &Config) -> BuckyResult<()> {
        if let Some(mysql_config) = &config.mysql {
            let mut options = MySqlConnectOptions::new()
                .host(&mysql_config.host)
                .port(mysql_config.port)
                .username(&mysql_config.username)
                .password(&mysql_config.password)
                .database(&mysql_config.db);
            options.log_statements(LevelFilter::Off);
            let pool = MySqlPoolOptions::new().max_connections(10).connect_with(options).await.map_err(map_sql_err)?;

            let _ = self.pool.set(pool);
            Ok(())
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidFormat, ""))
        }
    }

    async fn init(&self) -> BuckyResult<()> {
        sqlx::query(INIT_BLOCK_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_RECEIPT_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_RECEIPT_RAW_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_TX_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        sqlx::query(INIT_TX_RAW_TABLE).execute(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
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
            .bind(limit as u64)
            .bind((pages*limit) as u64)
            .fetch_all(self.pool.get().unwrap()).await.map_err(map_sql_err)?;

        let mut ret = Vec::new();
        for row in rows {
            let height = row.get("height");
            let txs = self.get_txids(height).await?;
            ret.push(BlockInfo {
                height,
                id: row.get("id"),
                create_time: row.get("create_time"),
                size: row.get("size"),
                fee: row.get("fee"),
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
        }.bind(limit as u64)
            .bind((pages*limit) as u64)
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
        let sum:i64 = row.try_get(0).map_err(|e| {
            error!("get tx sum err {} return default 0", e);
            e
        }).unwrap_or(0);
        Ok(AccountInfo {
            txs: sum as u64
        })
    }
}
