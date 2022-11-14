use crate::*;
use crate::state_storage::{State, NameExtra, DescExtra, Storage, storage_in_mem_path, StorageRef};
use async_trait::async_trait;
use cyfs_base::*;
use sqlx::{Row, Connection, ConnectOptions, Sqlite};
use crate::helper::get_meta_err_code;
use async_std::sync::{Mutex, MutexGuard, Arc};
use std::str::FromStr;
use log::*;
use sha2::{Sha256, Digest};
use std::path::{PathBuf, Path};
use std::time::Duration;
use crate::state_storage::state::AccountInfo;
use sqlx::sqlite::{SqliteJournalMode};
use primitive_types::H256;
const PEERID_LENGTH: u32 = 32;

pub struct SqlState {
    conn: Mutex<MetaConnection>,
    transaction_seq: Mutex<i32>,
}

pub type StateRef = std::sync::Arc<SqlState>;
pub type StateWeakRef = std::sync::Weak<SqlState>;

impl SqlState {
    pub fn new(conn: MetaConnection) -> StateRef {
        StateRef::new(SqlState {
            conn: Mutex::new(conn),
            transaction_seq: Mutex::new(0),
        })
    }

    pub async fn get_conn(&self) -> MutexGuard<'_, MetaConnection> {
        self.conn.lock().await
    }

    fn single_balance_tbl_name(&self, ctid: &CoinTokenId) -> String {
        match ctid {
            CoinTokenId::Coin(id) => format!("coin_single_{}", *id),
            CoinTokenId::Token(id) => format!("token_single_{}", id.to_hex().unwrap())
        }
    }

    fn union_balance_tbl_name(&self, ctid: &CoinTokenId) -> String {
        match ctid {
            CoinTokenId::Coin(id) => format!("coin_union_{}", *id),
            CoinTokenId::Token(id) => format!("token_union_{}", id.to_hex().unwrap())
        }
    }

    fn union_balance_col_pre_name(&self, which: &PeerOfUnion) -> &'static str {
        match which {
            PeerOfUnion::Left => "left",
            PeerOfUnion::Right => "right"
        }
    }

    async fn init_balance_tbl(&self, ctid: &CoinTokenId) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = format!("CREATE TABLE IF NOT EXISTS \"{}\"
            (\"id\" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
            \"balance\" INTEGER NOT NULL);", self.single_balance_tbl_name(ctid));
        conn.execute_sql(sqlx::query(sql.as_str())).await?;

        let sql = format!(r#"CREATE TABLE IF NOT EXISTS "{}_unpaid"
            ("id" INTEGER  PRIMARY KEY autoincrement,
             "account_id" CHAR(45) NOT NULL,
             "type" text NOT NULL,
             "to" CHAR(45) NOT NULL,
             "height" INTEGER NOT NULL,
             "amount" INTEGER NOT NULL);"#, self.single_balance_tbl_name(ctid));
        conn.execute_sql(sqlx::query(sql.as_str())).await?;

        let sql = format!("CREATE TABLE IF NOT EXISTS \"{}\"
            (\"id\" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
            \"left_balance\" INTEGER NOT NULL,
            \"right_balance\" INTEGER NOT NULL,
            \"deviation\" INTEGER NOT NULL,
            \"seq\" INTEGER NOT NULL);",
                          self.union_balance_tbl_name(ctid));
        conn.execute_sql(sqlx::query(sql.as_str())).await?;

        Ok(())
    }

    async fn init_obj_desc_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS all_descs (
	                "obj_id" char(45) NOT NULL UNIQUE,
	                "desc" BLOB NOT NULL,
	                "update_time" INTEGER,
	                PRIMARY KEY("obj_id"));"#;

        self.get_conn().await.execute_sql(sqlx::query(sql)).await?;
        return Ok(());
    }

    async fn init_benefi_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "address_benefi" (
            "address"	TEXT NOT NULL UNIQUE,
            "benefi"	TEXT NOT NULL,
            PRIMARY KEY("address")
        )"#;

        self.get_conn().await.execute_sql(sqlx::query(sql)).await?;
        return Ok(());
    }

    async fn init_desc_rent_state_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS desc_extra (
        "obj_id" char(45) PRIMARY KEY NOT NULL UNIQUE,
        "rent_arrears" INTEGER,
        "rent_arrears_count" INTEGER,
        "rent_value" INTEGER,
        "coin_id" INTEGER,
        "data_len" INTEGER,
        "other_charge_balance" INTEGER);"#;

        self.get_conn().await.execute_sql(sqlx::query(sql)).await?;
        return Ok(());
    }

    async fn init_name_info_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "all_names" (
            "name_id"	char(1024) NOT NULL,
            "name_info"	BLOB NOT NULL,
            "name_state"	INTEGER NOT NULL,
            "owner"	char(45) NOT NULL,
            "coin_id" u8,
            "rent_arrears" INTEGER,
            "rent_arrears_count" INTEGER,
            "rent_value" INTEGER,
            "buy_coin_id" INTEGER,
            "buy_price" INTEGER,
            PRIMARY KEY("name_id")
            )"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS owner_index ON all_names (owner)"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        return Ok(());
    }

    fn get_cycle_event_table_name(&self, cycle: i64) -> String {
        format!("cycle_event_{}", cycle)
    }

    async fn init_cycle_event_table(&self, cycle: i64) -> BuckyResult<()> {
        {
            let table_name = self.get_cycle_event_table_name(cycle);
            let sql = format!(r#"CREATE TABLE IF NOT EXISTS {} (
            "key"	char(45) NOT NULL,
            "height" INTEGER NOT NULL,
            "real_key" TEXT NOT NULL,
            "start_height" INTEGER NOT NULL,
            "param"	BLOB NOT NULL,
            PRIMARY KEY("key")
        )"#, table_name);

            let mut conn = self.get_conn().await;
            conn.execute_sql(sqlx::query(sql.as_str())).await?;

            let index_sql = format!(r#"CREATE INDEX IF NOT EXISTS height_index ON {} (height)"#, table_name);
            conn.execute_sql(sqlx::query(index_sql.as_str())).await?;
        }
        self.add_cycle(cycle).await?;

        return Ok(());
    }

    async fn init_event_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "event" (
            "type"	INTEGER NOT NULL,
            "key"	TEXT NOT NULL,
            "param"	BLOB NOT NULL,
            "height"	INTEGER NOT NULL,
            PRIMARY KEY("type","key","height")
        )"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS height_index ON event (height)"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;


        let sql = r#"create table if not exists event_cycles (
            "cycle" INTEGER PRIMARY KEY
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        return Ok(());
    }

    async fn init_once_event_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "once_event" (
            "key"	TEXT NOT NULL,
            "height"	INTEGER NOT NULL,
            "param"	BLOB NOT NULL,
            PRIMARY KEY("key")
        )"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS height_index ON once_event (height)"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        return Ok(());
    }

    fn account_tbl_name(&self) -> &'static str {
        static NONCE_TBL_NAME: &str = "account";
        NONCE_TBL_NAME
    }

    async fn init_account_tbl(&self) -> BuckyResult<()> {
        let sql = format!("CREATE TABLE IF NOT EXISTS \"{}\"
            (\"id\" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
            \"nonce\" INTEGER NOT NULL);", self.account_tbl_name());
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql.as_str())).await?;

        let sql = format!(r#"CREATE TABLE IF NOT EXISTS "account_info"
            ("id" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
             "data" BLOB NOT NULL)"#);
        conn.execute_sql(sqlx::query(sql.as_str())).await?;

        Ok(())
    }

    async fn init_code_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "code" (
            "id"	CHAR(64) NOT NULL,
            "code"	BLOB NOT NULL,
            PRIMARY KEY("id")
        )"#;

        self.get_conn().await.execute_sql(sqlx::query(sql)).await?;
        Ok(())
    }

    async fn init_storage_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "storage" (
            "id"	CHAR(64) NOT NULL,
            "key"	CHAR(64) NOT NULL,
            "value"	BLOB,
            PRIMARY KEY("key","id")
        )"#;

        self.get_conn().await.execute_sql(sqlx::query(sql)).await?;
        Ok(())
    }

    async fn init_config_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "config" (
            "key"	TEXT NOT NULL,
            "value"	TEXT NOT NULL,
            PRIMARY KEY("key")
        )"#;

        self.get_conn().await.execute_sql(sqlx::query(sql)).await?;
        return Ok(());
    }

    async fn init_service_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "services" (
            "service_id" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
            "service_status" INTEGER,
            "service" BLOB NOT NULL
        )"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"CREATE TABLE IF NOT EXISTS "contracts" (
        "contract_id" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
        "service_id" CHAR(45) NOT NULL,
        "buyer_id" CHAR(45) NOT NULL,
        "auth_type" INTEGER,
        "contract" BLOB NOT NULL,
        "auth_list" BLOB NOT NULL
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS buyer_index ON contracts (service_id, buyer_id)"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        Ok(())
    }

    async fn init_subchain_withdraw_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "subchain_withdraw" (
        "subchain_id" CHAR(45) NOT NULL,
        "withdraw_tx_id" CHAR(45) NOT NULL,
        "withdraw_data" BLOB NOT NULL
        )"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS withdraw_index ON subchain_withdraw (subchain_id, withdraw_tx_id)"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;
        Ok(())
    }

    async fn init_nft_table(&self) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = r#"create table if not exists nft (
            "object_id" char(45) PRIMARY KEY,
            "nft_label" char(45) NOT NULL,
            "desc" BLOB NOT NULL,
            "name" text NOT NULL,
            "state" BLOB NOT NULL
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create unique index if not exists nft_label_index on nft(nft_label)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create table if not exists nft_apply_buy (
            "id" INTEGER  PRIMARY KEY autoincrement,
            "nft_id" char(45) not null,
            "buyer_id" char(45) not null,
            "price" integer not null,
            "coin_id" blob not null
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create unique index if not exists nft_apply_buy_index1 on nft_apply_buy(nft_id, buyer_id)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_apply_buy_index2 on nft_apply_buy(nft_id, id)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create table if not exists nft_bid (
            "id" INTEGER  PRIMARY KEY autoincrement,
            "nft_id" char(45) not null,
            "buyer_id" char(45) not null,
            "price" integer not null,
            "coin_id" blob not null
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create unique index if not exists nft_bid_index1 on nft_bid(nft_id, buyer_id)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_bid_index2 on nft_bid(nft_id, id)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        Ok(())
    }
    async fn init_evm_log_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "evm_log" (
            "address"	TEXT NOT NULL,
            "block"	INTEGER NOT NULL,
            "topic0"	TEXT,
            "topic1"	TEXT,
            "topic2"	TEXT,
            "topic3"	TEXT,
            "data"	BLOB
        )"#;

        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)).await?;

        Ok(())
    }

    async fn get_config(&self, key: &str, default: Option<String>) -> BuckyResult<String> {
        let sql = "select value from config where key=?1";

        let ret = self.get_conn().await.query_one(sqlx::query(sql)
            .bind(key)).await;
        match ret {
            Ok(row) => Ok(row.get("value")),
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND && default.is_some() {
                    Ok(default.unwrap())
                } else {
                    Err(crate::meta_err!(ERROR_EXCEPTION))
                }
            }
        }
    }

    async fn add_cycle(&self, cycle: i64) -> BuckyResult<()> {
        if self.cycle_exist(cycle).await? {
            return Ok(());
        }
        let sql = "insert into event_cycles (cycle) values (?1)";
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(cycle)).await?;
        Ok(())
    }

    async fn cycle_exist(&self, cycle: i64) -> BuckyResult<bool> {
        let mut conn = self.get_conn().await;

        let sql = r#"create table if not exists event_cycles (
            "cycle" INTEGER PRIMARY KEY
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = "select * from event_cycles where cycle = ?1";
        let ret = conn.query_one(sqlx::query(sql).bind(cycle)).await;
        Ok(ret.is_ok())
    }
}

#[async_trait]
impl State for SqlState {
    async fn being_transaction(&self) -> BuckyResult<()> {
        let mut seq = self.transaction_seq.lock().await;
        let cur_seq = *seq;
        *seq += 1;
        let pos = if cur_seq == 0 {
            None
        } else {
            Some(format!("{}", cur_seq))
        };
        let mut conn = self.get_conn().await;
        let sql = MetaTransactionSqlCreator::begin_transaction_sql(pos);
        // println!("{}", sql.as_str());
        conn.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }

    async fn rollback(&self) -> BuckyResult<()> {
        let mut seq = self.transaction_seq.lock().await;
        let cur_seq = *seq;
        *seq -= 1;
        let pos = if cur_seq <= 1 {
            None
        } else {
            Some(format!("{}", seq))
        };
        let sql = MetaTransactionSqlCreator::rollback_transaction_sql(pos);
        // println!("{}", sql.as_str());
        self.get_conn().await.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }

    async fn commit(&self) -> BuckyResult<()> {
        let mut seq = self.transaction_seq.lock().await;
        let cur_seq = *seq;
        *seq -= 1;
        let pos = if cur_seq == 1 {
            None
        } else {
            Some(format!("{}", seq))
        };
        let sql = MetaTransactionSqlCreator::commit_transaction_sql(pos);
        // println!("{}", sql.as_str());
        self.get_conn().await.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }

    async fn init(&self) -> BuckyResult<()> {
        self.init_event_table().await?;
        self.init_desc_rent_state_table().await?;
        self.init_once_event_table().await?;
        self.init_service_tbl().await?;
        self.init_code_tbl().await?;
        self.init_storage_tbl().await?;
        self.init_benefi_table().await?;
        self.init_evm_log_table().await?;

        self.init_balance_tbl(&CoinTokenId::Coin(0)).await?;
        self.init_balance_tbl(&CoinTokenId::Coin(1)).await?;
        Ok(())
    }

    async fn create_cycle_event_table(&self, cycle: i64) -> BuckyResult<()> {
        self.init_cycle_event_table(cycle).await
    }

    async fn config_get(&self, key: &str, default: &str) -> BuckyResult<String> {
        let sql = "select value from config where key=?1";

        let ret = self.get_conn().await.query_one(sqlx::query(sql)
            .bind(key)).await;
        match ret {
            Ok(row) => Ok(row.get("value")),
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    Ok(default.to_owned())
                } else {
                    Err(crate::meta_err!(ERROR_EXCEPTION))
                }
            }
        }
    }

    async fn config_set(&self, key: &str, value: &str) -> BuckyResult<()> {
        let sql = "insert into config values (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2";
        self.get_conn().await.execute_sql(sqlx::query(sql).bind(key).bind(value)).await?;
        Ok(())
    }

    async fn init_genesis(&self, coins: &Vec<GenesisCoinConfig>) -> BuckyResult<()> {
        // init account
        self.init_account_tbl().await?;

        self.init_balance_tbl(&CoinTokenId::Coin(0)).await?;

        // init coins
        for coin in coins {
            self.init_balance_tbl(&CoinTokenId::Coin(coin.coin_id)).await?;
            for (account, balance) in &coin.pre_balance {
                self.modify_balance(&CoinTokenId::Coin(coin.coin_id), account, *balance).await?;
            }
        }

        // init obj-desc
        self.init_obj_desc_table().await?;

        // init name-info
        self.init_name_info_table().await?;

        self.init_config_tbl().await?;
        self.init_event_table().await?;
        self.init_desc_rent_state_table().await?;
        self.init_once_event_table().await?;
        self.init_subchain_withdraw_table().await?;
        self.init_nft_table().await?;
        Ok(())
    }

    async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<i64> {
        let sql = format!("SELECT nonce FROM {} WHERE id=?1", self.account_tbl_name());
        let ret = self.get_conn().await.query_one(sqlx::query(sql.as_str())
            .bind(account.to_string())).await;
        match ret {
            Ok(row) => Ok(row.get("nonce")),
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    Ok(0)
                } else {
                    Err(crate::meta_err!(ERROR_EXCEPTION))
                }
            }
        }
    }

    async fn inc_nonce(&self, account: &ObjectId) -> BuckyResult<i64> {
        let nonce;
        let qeury_sql = format!("SELECT nonce FROM {} WHERE id=?1", self.account_tbl_name());
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(qeury_sql.as_str()).bind(account.to_string())).await;

        if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                nonce = 1;
                let insert_sql = format!("INSERT INTO {} (id, nonce) VALUES (?1, ?2)", self.account_tbl_name());
                conn.execute_sql(sqlx::query(insert_sql.as_str()).bind(account.to_string()).bind(nonce)).await?;
            } else {
                return Err(crate::meta_err!(ERROR_EXCEPTION));
            }
        } else {
            nonce = query_result.unwrap().get::<i64, &str>("nonce") + 1;
            let update_sql = format!("UPDATE {} SET nonce=?1 WHERE id=?2", self.account_tbl_name());
            conn.execute_sql(sqlx::query(update_sql.as_str()).bind(nonce).bind(account.to_string())).await?;
        }

        Ok(nonce)
    }

    async fn set_nonce(&self, account: &ObjectId, nonce: i64) -> BuckyResult<()> {
        let qeury_sql = format!("SELECT nonce FROM {} WHERE id=?1", self.account_tbl_name());
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(qeury_sql.as_str()).bind(account.to_string())).await;

        if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                let insert_sql = format!("INSERT INTO {} (id, nonce) VALUES (?1, ?2)", self.account_tbl_name());
                conn.execute_sql(sqlx::query(insert_sql.as_str()).bind(account.to_string()).bind(nonce)).await?;
            } else {
                return Err(crate::meta_err!(ERROR_EXCEPTION));
            }
        } else {
            let update_sql = format!("UPDATE {} SET nonce=?1 WHERE id=?2", self.account_tbl_name());
            conn.execute_sql(sqlx::query(update_sql.as_str()).bind(nonce).bind(account.to_string())).await?;
        }

        Ok(())
    }

    async fn add_account_info(&self, info: &AccountInfo) -> BuckyResult<()> {
        let id = info.get_id();
        let ret = self.get_account_info(&id).await;
        if let Err(err) = ret {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                let insert_sql = "INSERT INTO account_info (id, data) VALUES (?1, ?2)";
                let mut conn = self.get_conn().await;
                conn.execute_sql(sqlx::query(insert_sql).bind(info.get_id().to_string()).bind(info.to_vec()?)).await?;
            } else {
                return Err(err);
            }
        }
        Ok(())
    }

    async fn get_account_info(&self, account: &ObjectId) -> BuckyResult<AccountInfo> {
        let sql = "select * from account_info where id = ?1";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(account.to_string())).await?;
        let data: Vec<u8> = row.get("data");
        let info = AccountInfo::clone_from_slice(data.as_slice())?;
        Ok(info)
    }

    async fn get_account_permission(&self, _account: &ObjectId) -> BuckyResult<u32> {
        unimplemented!()
    }

    async fn get_balance(&self, account: &ObjectId, ctid: &CoinTokenId) -> BuckyResult<i64> {
        let sql = format!("SELECT balance FROM {} WHERE id=?1", self.single_balance_tbl_name(ctid));
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(sql.as_str()).bind(account.to_string())).await;
        if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                Ok(0)
            } else {
                Err(meta_err!(ERROR_EXCEPTION))
            }
        } else {
            let ret = query_result.unwrap();
            let mut balance = ret.try_get::<i64, &str>("balance");
            if balance.is_err() {
                balance = ret.try_get::<f64, &str>("balance").and_then(|v| {
                    Ok(v as i64)
                });
            }

            balance.map_err(|e|{
                error!("get {} balance err {}", account, e);
                meta_err!(ERROR_EXCEPTION)
            })
        }
    }

    async fn modify_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()> {
        let sql = format!("REPLACE INTO {} (balance, id) VALUES (?1, ?2)", self.single_balance_tbl_name(ctid));
        let mut conn = self.get_conn().await;
        let changed = conn.execute_sql(sqlx::query(&sql).bind(v).bind(account.to_string())).await?;
        if changed.rows_affected() != 1 {
            Err(crate::meta_err!(ERROR_EXCEPTION))
        } else {
            Ok(())
        }
    }

    async fn inc_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()> {
        if v == 0 {
            return Ok(());
        }
        let update_sql = format!("UPDATE {} SET balance=balance+?1 WHERE id=?2", self.single_balance_tbl_name(ctid));
        let mut conn = self.get_conn().await;
        let changed = conn.execute_sql(sqlx::query(&update_sql).bind(v).bind(account.to_string())).await?;
        if changed.rows_affected() != 1 {
            let insert_sql = format!("INSERT INTO {} (balance, id) VALUES (?1, ?2)", self.single_balance_tbl_name(ctid));
            let changed = conn.execute_sql(sqlx::query(&insert_sql).bind(v).bind(account.to_string())).await?;
            if changed.rows_affected() != 1 {
                return Err(crate::meta_err!(ERROR_EXCEPTION));
            }
        }
        Ok(())
    }

    async fn dec_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()> {
        if v == 0 {
            return Ok(());
        }
        let sql = format!("UPDATE {} SET balance=balance-?1 WHERE id=?2 AND balance>=?1", self.single_balance_tbl_name(ctid));
        let mut conn = self.get_conn().await;
        let changed = conn.execute_sql(sqlx::query(&sql).bind(v).bind(account.to_string())).await?;
        if changed.rows_affected() != 1 {
            warn!("dec {} balance {} fail", account, v);
            Err(crate::meta_err!(ERROR_NO_ENOUGH_BALANCE))
        } else {
            Ok(())
        }
    }

    async fn issue_token(&self, _to: &ObjectId, _v: u64, token_id: &ObjectId) -> BuckyResult<()> {
        self.init_balance_tbl(&CoinTokenId::Token(*token_id)).await
    }

    async fn get_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId) -> BuckyResult<UnionBalance> {
        let total = self.get_balance(union, ctid).await?;
        let sql = format!("SELECT left_balance, right_balance, deviation FROM {} WHERE id=?1", self.union_balance_tbl_name(ctid));
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(&sql).bind(union.to_string())).await;
        if let Err(err) = query_result {
            warn!("query union balance err {}", err);
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                Ok(UnionBalance::default())
            } else {
                Err(crate::meta_err!(ERROR_EXCEPTION))
            }
        } else {
            let row = query_result.unwrap();

            Ok(UnionBalance {
                total,
                left: row.get("left_balance"),
                right: row.get("right_balance"),
                deviation: row.get("deviation")
            })
        }
    }

    async fn get_union_deviation_seq(&self, ctid: &CoinTokenId, union: &ObjectId) -> BuckyResult<i64> {
        let sql = format!("SELECT seq FROM {} WHERE id=?1", self.union_balance_tbl_name(ctid));
        let mut conn = self.get_conn().await;
           let query_result = conn.query_one(sqlx::query(sql.as_str()).bind(union.to_string())).await;
        if let Err(e) = query_result {
            error!("get deviation seq err {}", e);
            Err(crate::meta_err!(ERROR_EXCEPTION))
        } else {
            Ok(query_result.unwrap().get("seq"))
        }
    }

    async fn update_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId, balance: &UnionBalance) -> BuckyResult<()> {
        let sql = format!("UPDATE {} SET left_balance=?1, right_balance=?2, deviation=?3 WHERE id=?4", self.union_balance_tbl_name(ctid));

        let mut conn = self.get_conn().await;
        let changed = conn.execute_sql(sqlx::query(&sql)
            .bind(balance.left)
            .bind(balance.right)
            .bind(balance.deviation)
            .bind(union.to_string())).await?;
        if changed.rows_affected() != 1 {
            Err(crate::meta_err!(ERROR_EXCEPTION))
        } else {
            Ok(())
        }
    }

    async fn deposit_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId, from: PeerOfUnion, v: i64) -> BuckyResult<()> {
        let pre_name = self.union_balance_col_pre_name(&from);
        let update_sql = format!("UPDATE {} SET {}_balance={}_balance+?1 WHERE id=?2", self.union_balance_tbl_name(ctid), pre_name, pre_name);
        let mut conn = self.get_conn().await;
        let changed = conn.execute_sql(sqlx::query(update_sql.as_str()).bind(v).bind(union.to_string())).await?;
        if changed.rows_affected() != 1 {
            let insert_sql = match from {
                PeerOfUnion::Left => format!("INSERT INTO {} (id, left_balance, right_balance, deviation, seq) VALUES (?1, ?2, 0, 0, -1)", self.union_balance_tbl_name(ctid)),
                PeerOfUnion::Right => format!("INSERT INTO {} (id, right_balance, left_balance, deviation, seq) VALUES (?1, ?2, 0, 0, -1)", self.union_balance_tbl_name(ctid))
            };
            let changed = conn.execute_sql(sqlx::query(insert_sql.as_str()).bind(union.to_string()).bind(v)).await?;
            if changed.rows_affected() != 1 {
                return Err(crate::meta_err!(ERROR_EXCEPTION));
            }
        }
        Ok(())
    }

    async fn withdraw_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId, to: PeerOfUnion, withdraw: i64) -> BuckyResult<i64> {
        let mut balance = self.get_union_balance(ctid, union).await?;
        // 从union出得手续费会导致total减少
        let spent = (balance.left + balance.right) - balance.total;
        let mut left_spent = (spent / 2) as i64;
        // 修正left 和 right得本金
        if balance.left > left_spent {
            balance.left-= left_spent;
        } else {
            balance.left = 0;
            left_spent = balance.left;
        }
        let right_spent = spent - left_spent;
        balance.right -= right_spent;

        return match to {
            PeerOfUnion::Left => {
                let left_balance = balance.left + balance.deviation;
                if left_balance >= withdraw {
                    balance.left -= withdraw;
                    balance.total -= withdraw;
                    self.update_union_balance(ctid, union, &balance).await?;
                    Ok(left_balance)
                } else {
                    warn!("withdraw left {}", left_balance);
                    Err(crate::meta_err!(ERROR_NO_ENOUGH_BALANCE))
                }
            },
            PeerOfUnion::Right => {
                let right_balance = balance.right - balance.deviation;
                if right_balance >= withdraw {
                    balance.right -= withdraw;
                    balance.total -= withdraw;
                    self.update_union_balance(ctid, union,  &balance).await?;
                    Ok(right_balance)
                } else {
                    warn!("withdraw right {}", right_balance);
                    Err(crate::meta_err!(ERROR_NO_ENOUGH_BALANCE))
                }
            }
        };
    }

    async fn update_union_deviation(&self, ctid: &CoinTokenId, union: &ObjectId, deviation: i64, seq: i64) -> BuckyResult<()> {
        let old_seq = self.get_union_deviation_seq(ctid, union).await?;
        if old_seq >= seq {
            return Err(crate::meta_err!(ERROR_ACCESS_DENIED));
        }

        let balance = self.get_union_balance(ctid, union).await?;
        if balance.left + deviation < 0 || balance.right - deviation < 0 {
            return Err(crate::meta_err!(ERROR_NO_ENOUGH_BALANCE));
        }
        let update_sql = format!("UPDATE {} SET deviation=?1, seq=?2 WHERE id=?3", self.union_balance_tbl_name(ctid));
        let mut conn = self.get_conn().await;
        let changed = conn.execute_sql(sqlx::query(update_sql.as_str()).bind(deviation).bind(seq).bind(union.to_string())).await?;
        if changed.rows_affected() != 1 {
            return Err(crate::meta_err!(ERROR_NOT_FOUND));
        }
        Ok(())
    }

    async fn get_desc_extra(&self, id: &ObjectId) -> BuckyResult<DescExtra> {
        let sql = "SELECT rent_arrears, rent_value, coin_id, data_len, other_charge_balance, rent_arrears_count from desc_extra where obj_id=?1";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(id.to_string())).await?;
        Ok(DescExtra {
            obj_id: id.clone(),
            rent_arrears: row.get("rent_arrears"),
            rent_value: row.get("rent_value"),
            coin_id: row.get::<i16, &str>("coin_id") as u8,
            data_len: row.get("data_len"),
            other_charge_balance: row.get("other_charge_balance"),
            rent_arrears_count: row.get("rent_arrears_count"),
        })
    }

    async fn add_or_update_desc_extra(&self, state: &DescExtra) -> BuckyResult<()> {
        let sql = r#"INSERT INTO desc_extra (obj_id, rent_arrears, rent_value, coin_id, data_len, other_charge_balance, rent_arrears_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(obj_id) DO UPDATE SET rent_arrears=?2, rent_value=?3, coin_id=?4, data_len=?5, other_charge_balance=?6, rent_arrears_count=?7"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(
            sqlx::query(sql)
            .bind(state.obj_id.to_string())
            .bind(state.rent_arrears)
            .bind(state.rent_value)
            .bind(state.coin_id as i16)
            .bind(state.data_len)
            .bind(state.other_charge_balance)
            .bind(state.rent_arrears_count)).await?;
         Ok(())
    }

    async fn add_or_update_desc_rent_state(&self, state: &DescExtra) -> BuckyResult<()> {
        let sql = r#"INSERT INTO desc_extra (obj_id, rent_arrears, rent_value, coin_id, data_len, other_charge_balance, rent_arrears_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(obj_id) DO UPDATE SET rent_arrears=?2, rent_value=?3, coin_id=?4, data_len=?5, rent_arrears_count=?7"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)
            .bind(state.obj_id.to_string())
            .bind(state.rent_arrears)
            .bind(state.rent_value)
            .bind(state.coin_id as i16)
            .bind(state.data_len)
            .bind(state.other_charge_balance)
            .bind(state.rent_arrears_count)).await?;
        Ok(())
    }

    async fn add_or_update_desc_other_charge_balance(&self, state: &DescExtra) -> BuckyResult<()> {
        let sql = r#"INSERT INTO desc_extra (obj_id, rent_arrears, rent_value, coin_id, data_len, other_charge_balance, rent_arrears_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(obj_id) DO UPDATE SET other_charge_balance=?6"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)
                             .bind(state.obj_id.to_string())
                             .bind(state.rent_arrears)
                             .bind(state.rent_value)
                             .bind(state.coin_id as i16)
                             .bind(state.data_len)
                             .bind(state.other_charge_balance)
                             .bind(state.rent_arrears_count)).await?;

        Ok(())
    }

    async fn update_desc_extra(&self, state: &DescExtra) -> BuckyResult<()> {
        let sql = "UPDATE desc_extra set rent_arrears=?1, rent_value=?2, coin_id=?3, data_len=?4, other_charge_balance=?5 where obj_id=?6";
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)
                             .bind(state.rent_arrears)
                             .bind(state.rent_value)
                             .bind(state.coin_id as i16)
                             .bind(state.data_len)
                             .bind(state.other_charge_balance)
                             .bind(state.obj_id.to_string())).await?;
        Ok(())
    }

    async fn drop_desc_extra(&self, obj_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from desc_extra where obj_id=?1";let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(obj_id.to_string())).await?;
        Ok(())
    }

    async fn get_name_extra(&self, id: &str) -> BuckyResult<NameExtra> {
        let sql = "SELECT rent_arrears, rent_value, owner, coin_id, buy_price, buy_coin_id, rent_arrears_count from all_names where name_id=?1";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(id.to_string())).await?;
        Ok(NameExtra {
            name_id: id.to_string(),
            rent_arrears: row.get("rent_arrears"),
            rent_value: row.get("rent_value"),
            coin_id: row.get::<i16, &str>("coin_id") as u8,
            owner: ObjectId::from_str(row.get("owner"))?,
            buy_price: row.get("buy_price"),
            buy_coin_id: row.get::<i16, &str>("buy_coin_id") as u8,
            rent_arrears_count: row.get("rent_arrears_count"),
        })
    }

    async fn add_or_update_name_extra(&self, state: &NameExtra) -> BuckyResult<()> {
        let sql = "UPDATE all_names set rent_arrears=?1, rent_arrears_count=?6, rent_value=?2, buy_price=?3, buy_coin_id=?5 where name_id=?4";
        let mut conn = self.get_conn().await;
        let done = conn.execute_sql(sqlx::query(sql)
            .bind(state.rent_arrears)
            .bind(state.rent_value)
            .bind(state.buy_price)
            .bind(state.name_id.to_string())
            .bind(state.buy_coin_id as i16)
            .bind(state.rent_arrears_count)).await?;
        if done.rows_affected() == 0 {
            let sql = "INSERT INTO all_names (name_id, owner, rent_arrears, rent_arrears_count, rent_value, buy_price, buy_coin_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
            conn.execute_sql(sqlx::query(sql)
                .bind(state.name_id.to_string())
                .bind(state.owner.to_string())
                .bind(state.rent_arrears)
                .bind(state.rent_arrears_count)
                .bind(state.rent_value)
                .bind(state.buy_price)
                .bind(state.buy_coin_id as i16)).await?;
        }
        Ok(())
    }

    async fn add_or_update_name_rent_state(&self, state: &NameExtra) -> BuckyResult<()> {
        let sql = "UPDATE all_names set rent_arrears=?, rent_arrears_count=?, rent_value=? where name_id=?";
        let mut conn = self.get_conn().await;
        let done = conn.execute_sql(sqlx::query(sql)
            .bind(state.rent_arrears)
            .bind(state.rent_arrears_count)
            .bind(state.rent_value)
            .bind(state.name_id.to_string())).await?;
        if done.rows_affected() == 0 {
            let sql = "INSERT INTO all_names (name_id, owner, rent_arrears, rent_arrears_count, rent_value, buy_price, buy_coin_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
            conn.execute_sql(sqlx::query(sql)
                .bind(state.name_id.to_string())
                .bind(state.owner.to_string())
                .bind(state.rent_arrears)
                .bind(state.rent_arrears_count)
                .bind(state.rent_value)
                .bind(state.buy_price)
                .bind(state.buy_coin_id as i16)).await?;
        }
        Ok(())
    }

    async fn add_or_update_name_buy_price(&self, state: &NameExtra) -> BuckyResult<()> {
        let sql = "UPDATE all_names set buy_price=? where name_id=?";
        let mut conn = self.get_conn().await;
        let done = conn.execute_sql(sqlx::query(sql)
            .bind(state.buy_price)
            .bind(state.name_id.to_string())).await?;
        if done.rows_affected() == 0 {
            let sql = "INSERT INTO all_names (name_id, owner, rent_arrears, rent_arrears_count, rent_value, buy_price, buy_coin_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
            conn.execute_sql(sqlx::query(sql)
                .bind(state.name_id.to_string())
                .bind(state.owner.to_string())
                .bind(state.rent_arrears)
                .bind(state.rent_arrears_count)
                .bind(state.rent_value)
                .bind(state.buy_price)
                .bind(state.buy_coin_id as i16)).await?;
        }
        Ok(())
    }

    async fn create_name_info(&self, name: &str, info: &NameInfo) -> BuckyResult<()> {
        let sql = "SELECT name_state FROM all_names WHERE name_id=?1";
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(sql).bind(name)).await;


        return if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                let sql = r#"INSERT INTO all_names (name_id,name_info,name_state,owner,coin_id,rent_arrears
        ,rent_value,buy_coin_id,buy_price) VALUES (?1,?2,?3,?4,0, 0, 0, 0, 0)"#;
                let name_info_data_raw = info.to_vec();
                if name_info_data_raw.is_err() {
                    log::error!("serialize name info error!");
                    return Err(crate::meta_err!(ERROR_PARAM_ERROR));
                }
                let name_info_data_raw = name_info_data_raw.unwrap();
                // Create的时候，Name一定是Auction1状态
                let owner = if info.owner.is_some() {
                    info.owner.unwrap().to_string()
                } else {
                    String::from("")
                };
                let insert_result = conn.execute_sql(sqlx::query(&sql)
                    .bind(name)
                    .bind(name_info_data_raw)
                    .bind(NameState::Auction as i32)
                    .bind(owner)).await?;

                if insert_result.rows_affected() != 1 {
                    return Err(crate::meta_err!(ERROR_ALREADY_EXIST));
                }
                Ok(())
            } else {
                Err(crate::meta_err!(ERROR_EXCEPTION))
            }
        } else {
            Err(crate::meta_err!(ERROR_ALREADY_EXIST))
        }
    }

    async fn get_name_info(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>> {
        let sql = "SELECT name_info,name_state FROM all_names WHERE name_id=?1";
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(sql).bind(name)).await;
        return if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                Ok(None)
            } else {
                Err(crate::meta_err!(ERROR_EXCEPTION))
            }
        } else {
            let row = query_result.unwrap();
            let name_info_data: Vec<u8> = row.get("name_info");
            let name_state: i32 = row.get("name_state");
            let state = NameState::from(name_state);
            let name_info_result : NameInfo = NameInfo::clone_from_slice(name_info_data.as_slice())?;
            Ok(Some((name_info_result,state)))
        }

    }

    async fn get_owned_names(&self, owner: &ObjectId) -> BuckyResult<Vec<String>> {
        let sql = "select name_id from all_names where owner=?1";
        let mut conn = self.get_conn().await;
        let rows = conn.query_all(sqlx::query(sql).bind(owner.to_string())).await.map_err(|_| {
            crate::meta_err!(ERROR_EXCEPTION)
        })?;
        let mut ret = vec![];
        for row in rows {
            ret.push(row.get("name_id"));
        }
        Ok(ret)
    }

    async fn get_name_state(&self, name: &str) -> BuckyResult<NameState> {
        let sql = "select name_state from all_names where name_id=?";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(name)).await?;

        let name_state: i32 = row.get("name_state");
        let state = NameState::from(name_state);
        return Ok(state);
    }

    async fn update_name_info(&self, name: &str, info: &NameInfo) -> BuckyResult<()> {
        let sql = "UPDATE all_names SET name_info=?1, owner=?3 WHERE name_id=?2";
        let name_info_data_raw = info.to_vec();
        if name_info_data_raw.is_err() {
            log::error!("serialize name info error!");
            return Err(crate::meta_err!(ERROR_PARAM_ERROR));
        }
        let name_info_data_raw = name_info_data_raw.unwrap();
        let mut owner = "".to_owned();
        if info.owner.is_some() {
            owner = info.owner.unwrap().to_string();
        }

        let mut conn = self.get_conn().await;
        let done = conn.execute_sql(sqlx::query(sql)
            .bind(name_info_data_raw)
            .bind(name)
            .bind(owner)).await?;

        return if done.rows_affected() != 1 {
            Err(crate::meta_err!(ERROR_NOT_FOUND))
        } else {
            Ok(())
        }
    }

    async fn update_name_state(&self, name: &str, state: NameState) -> BuckyResult<()> {
        let sql = "UPDATE all_names SET name_state=?1 WHERE name_id=?2";
        let mut conn = self.get_conn().await;
        let done = conn.execute_sql(sqlx::query(sql).bind(state as i32).bind(name)).await?;
        return if done.rows_affected() != 1 {
            Err(crate::meta_err!(ERROR_NOT_FOUND))
        } else {
            Ok(())
        }
    }

    async fn update_name_rent_arrears(&self, name: &str, rent_arrears: i64) -> BuckyResult<()> {
        let sql = "UPDATE all_names SET rent_arrears=?1 WHERE name_id=?2";
        let mut conn = self.get_conn().await;
        let done = conn.execute_sql(sqlx::query(sql).bind(rent_arrears).bind(name)).await?;
        return if done.rows_affected() != 1 {
            Err(crate::meta_err!(ERROR_NOT_FOUND))
        } else {
            Ok(())
        }
    }

    async fn create_obj_desc(&self, objid: &ObjectId, desc: &SavedMetaObject) -> BuckyResult<()> {
        let sql = "SELECT update_time FROM all_descs WHERE obj_id=?1";
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(sql).bind(objid.to_string())).await;

        return if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                //TODO:没有正确的插入desc的更新时间
                let desc_data = desc.to_vec();
                if desc_data.is_err() {
                    log::error!("serialize desc error!");
                    return Err(crate::meta_err!(ERROR_PARAM_ERROR));
                }
                let desc_data_raw = desc_data.unwrap();
                let insert_sql = "INSERT INTO all_descs (obj_id,desc,update_time) VALUES (?1,?2,date('now'))";
                conn.execute_sql(sqlx::query(insert_sql).bind(objid.to_string()).bind(desc_data_raw)).await?;
                Ok(())
            } else {
                Err(crate::meta_err!(ERROR_EXCEPTION))
            }
        } else {
            Err(crate::meta_err!(ERROR_ALREADY_EXIST))
        }
    }

    async fn get_obj_desc(&self, objid: &ObjectId) -> BuckyResult<SavedMetaObject> {
        let sql = "SELECT desc FROM all_descs WHERE obj_id=?1";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(objid.to_string())).await?;

        let desc_data: Vec<u8> = row.get("desc");
        let desc_result = SavedMetaObject::clone_from_slice(desc_data.as_slice()).map_err(|_| {
            crate::meta_err!(ERROR_NOT_FOUND)
        })?;
        return Ok(desc_result);

    }

    async fn update_obj_desc(&self, objid: &ObjectId, desc: &SavedMetaObject, _flags: u8) -> BuckyResult<()> {
        let sql = "UPDATE all_descs SET desc=?1 WHERE obj_id=?2";
        let desc_data = desc.to_vec();
        if desc_data.is_err() {
            log::error!("serialize desc error!");
            return Err(crate::meta_err!(ERROR_PARAM_ERROR));
        }
        let desc_data_raw = desc_data.unwrap();
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(desc_data_raw).bind(objid.to_string())).await?;

        return Ok(());
    }

    async fn drop_desc(&self, obj_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from all_descs where obj_id=?1";
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(obj_id.to_string())).await?;
        Ok(())
    }

    async fn add_or_update_cycle_event(&self, key: &str, event: &Event, cycle: i64, start_height: i64) -> BuckyResult<()> {
        let table_name = self.get_cycle_event_table_name(cycle);
        let offset = start_height % cycle;

        let sql = format!(r#"insert into {} (key, height, start_height, real_key, param) values (?1, ?2, ?3, ?4, ?5)
        on conflict(key) do update set height = ?2, start_height = ?3, real_key = ?4, param = ?5"#, table_name);

        if key.len() > 64 {
            let mut hasher = Sha256::new();
            hasher.input(key);
            let hash_key = hex::encode(hasher.result());
            let content = event.get_content()?;
            let mut conn = self.get_conn().await;
            conn.execute_sql(sqlx::query(sql.as_str())
                .bind(hash_key)
                .bind(offset)
                .bind(start_height)
                .bind(key).bind(content)).await?;
        } else {
            let mut conn = self.get_conn().await;
            conn.execute_sql(sqlx::query(sql.as_str())
                .bind(key)
                .bind(offset)
                .bind(start_height)
                .bind(key)
                .bind(event.get_content()?)).await?;
        }
        Ok(())
    }

    async fn get_cycle_events(&self, offset: i64, cycle: i64) -> BuckyResult<Vec<(String, i64, Event)>> {
        let table_name = self.get_cycle_event_table_name(cycle);

        let sql = format!("select param, start_height, real_key from {} where height=?1", table_name);
        let mut conn = self.get_conn().await;
        let rows = conn.query_all(sqlx::query(sql.as_str()).bind(offset)).await?;
        let mut event_list = vec![];
        for row in rows {
            let event_data:Vec<u8> = row.get("param");
            let start_height: i64 = row.get("start_height");
            let real_key: String = row.get("real_key");
            let event: Event = Event::clone_from_slice(event_data.as_slice())?;
            event_list.push((real_key, start_height, event))
        }
        Ok(event_list)
    }

    async fn get_all_cycle_events(&self, cycle: i64) -> BuckyResult<Vec<(String, i64, Event)>> {
        let table_name = self.get_cycle_event_table_name(cycle);

        let sql = format!("select param, start_height, real_key from {}", table_name);
        let mut conn = self.get_conn().await;
        let rows = conn.query_all(sqlx::query(sql.as_str())).await?;
        let mut event_list = vec![];
        for row in rows {
            let event_data:Vec<u8> = row.get("param");
            let start_height: i64 = row.get("start_height");
            let real_key: String = row.get("real_key");
            let event: Event = Event::clone_from_slice(event_data.as_slice())?;
            event_list.push((real_key, start_height, event));
        }
        Ok(event_list)
    }

    async fn get_cycle_event_by_key(&self, key: &str, cycle: i64) -> BuckyResult<Event> {
        let table_name = self.get_cycle_event_table_name(cycle);

        let sql = format!("select param from {} where key=?1", table_name);
        let mut conn = self.get_conn().await;
        if key.len() > 64 {
            let mut hasher = Sha256::new();
            hasher.input(key);
            let hash_key = hex::encode(hasher.result());
            let row = conn.query_one(sqlx::query(sql.as_str()).bind(hash_key)).await?;

            let event_data:Vec<u8> = row.get("param");
            let event: Event = Event::clone_from_slice(event_data.as_slice())?;
            Ok(event)
        } else {
            let row = conn.query_one(sqlx::query(sql.as_str()).bind(key)).await?;

            let event_data:Vec<u8> = row.get("param");
            let event: Event = Event::clone_from_slice(event_data.as_slice())?;
            Ok(event)
        }
    }

    async fn get_cycle_event_by_key2(&self, key: &str, cycle: i64) -> BuckyResult<(i64, Event)> {
        let table_name = self.get_cycle_event_table_name(cycle);

        let sql = format!("select * from {} where key=?1", table_name);
        let mut conn = self.get_conn().await;
        if key.len() > 64 {
            let mut hasher = Sha256::new();
            hasher.input(key);
            let hash_key = hex::encode(hasher.result());
            let row = conn.query_one(sqlx::query(sql.as_str()).bind(hash_key)).await?;

            let event_data:Vec<u8> = row.get("param");
            let event: Event = Event::clone_from_slice(event_data.as_slice())?;
            let start_height: i64 = row.get("start_height");
            Ok((start_height, event))
        } else {
            let row = conn.query_one(sqlx::query(sql.as_str()).bind(key)).await?;

            let event_data:Vec<u8> = row.get("param");
            let event: Event = Event::clone_from_slice(event_data.as_slice())?;
            let start_height: i64 = row.get("start_height");
            Ok((start_height, event))
        }
    }

    async fn drop_cycle_event(&self, key: &str, cycle: i64) -> BuckyResult<()> {
        let table_name = self.get_cycle_event_table_name(cycle);
        let sql = format!("delete from {} where key=?1", table_name);

        if key.len() > 64 {
            let mut hasher = Sha256::new();
            hasher.input(key);
            let hash_key = hex::encode(hasher.result());
            let mut conn = self.get_conn().await;
            conn.execute_sql(sqlx::query(sql.as_str()).bind(hash_key)).await?;
        } else {
            let mut conn = self.get_conn().await;
            conn.execute_sql(sqlx::query(sql.as_str()).bind(key)).await?;
        }

        Ok(())
    }

    async fn drop_all_cycle_events(&self, cycle: i64) -> BuckyResult<()> {
        let table_name = self.get_cycle_event_table_name(cycle);
        let sql = format!("delete from {}", table_name);

        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }

    async fn add_or_update_event(&self, key: &str, event: Event, height: i64) -> BuckyResult<()> {
        let sql = "insert into event VALUES(?1, ?2, ?3, ?4) ON CONFLICT(type, key,height) DO UPDATE SET param=?3,height=?4";
        let event_raw = event.get_content()?;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(event.get_type() as i8).bind(key).bind(event_raw).bind(height)).await?;
        Ok(())
    }

    async fn get_event(&self, event_type: EventType, height: i64) -> BuckyResult<Vec<Event>> {
        let sql = "select param from event where type=?1 and height=?2";
        let mut conn = self.get_conn().await;
        let rows = conn.query_all(sqlx::query(sql).bind(event_type as i32).bind(height)).await?;
        let mut ret = vec![];
        for row in rows {
            let event_data:Vec<u8> = row.get("param");
            let event = Event::clone_from_slice(event_data.as_slice())?;
            ret.push(event);
        }
        Ok(ret)
    }

    async fn get_event_by_key(&self, key: &str, event_type: EventType) -> BuckyResult<Vec<(Event, i64)>> {
        let sql = "select param,height from event where type=?1 and key=?2";
        let mut conn = self.get_conn().await;
        let rows = conn.query_all(sqlx::query(sql).bind(event_type as i32).bind(key)).await?;
        let mut ret = vec![];
        for row in rows {
            let event_data:Vec<u8> = row.get("param");
            let height:i64 = row.get("height");
            let event = Event::clone_from_slice(event_data.as_slice())?;
            ret.push((event, height));
        }
        Ok(ret)
    }

    async fn drop_event(&self, height: i64) -> BuckyResult<()> {
        let sql = "delete from event where height<=?1";
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(height)).await?;
        Ok(())
    }

    async fn add_or_update_once_event(&self, key: &str, event: &Event, height: i64) -> BuckyResult<()> {
        let sql = "insert into once_event (key, height, param) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET param=?3,height=?2";
        let event_raw = event.get_content()?;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(key).bind(height).bind(event_raw)).await?;
        Ok(())
    }

    async fn get_once_events(&self, height: i64) -> BuckyResult<Vec<Event>> {
        let sql = "select param from once_event where height=?1";
        let mut conn = self.get_conn().await;
        let rows = conn.query_all(sqlx::query(sql).bind(height)).await?;
        let mut ret = vec![];
        for row in rows {
            let event_data:Vec<u8> = row.get("param");
            let event = Event::clone_from_slice(event_data.as_slice())?;
            ret.push(event);
        }
        Ok(ret)
    }

    async fn get_once_event_by_key(&self, key: &str) -> BuckyResult<Event> {
        let sql = "select param from once_event where key=?1";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(key)).await?;
        let event_data:Vec<u8> = row.get("param");
        let event: Event = Event::clone_from_slice(event_data.as_slice())?;
        Ok(event)
    }

    async fn drop_once_event(&self, height: i64) -> BuckyResult<()> {
        let sql = "delete from once_event where height<=?1";
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(height)).await?;
        Ok(())
    }

    async fn drop_once_event_by_key(&self, key: &str) -> BuckyResult<()> {
        let sql = "delete from once_event where key=?1";
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(key)).await?;
        Ok(())
    }

    async fn create_subchain_withdraw_record(&self, subchain_id: &ObjectId, withdraw_tx_id: &ObjectId, record: Vec<u8>) -> BuckyResult<()> {
        let sql = r#"insert into subchain_withdraw (subchain_id, withdraw_tx_id, withdraw_data) values (?1, ?2, ?3)"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)
            .bind(subchain_id.to_string())
            .bind(withdraw_tx_id.to_string())
            .bind(record)).await?;
        Ok(())
    }

    async fn update_subchain_withdraw_record(&self, subchain_id: &ObjectId, withdraw_tx_id: &ObjectId, record: Vec<u8>) -> BuckyResult<()> {
        let sql = r#"update subchain_withdraw set withdraw_data=?3 where subchain_id=?1 and withdraw_tx_id=?2"#;
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql)
            .bind(subchain_id.to_string())
            .bind(withdraw_tx_id.to_string())
            .bind(record)).await?;
        Ok(())
    }

    async fn get_subchain_withdraw_record(&self, subchain_id: &ObjectId, withdraw_tx_id: &ObjectId) -> BuckyResult<Vec<u8>> {
        let sql = r#"select withdraw_data from subchain_withdraw where subchain_id=?1 and withdraw_tx_id=?2"#;
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql)
            .bind(subchain_id.to_string())
            .bind(withdraw_tx_id.to_string())).await?;
        Ok(row.get("withdraw_data"))
    }

    async fn add_unpaid_record(&self, record: &UnpaidRecord) -> BuckyResult<()> {
        let sql = format!(r#"insert into {}_unpaid (account_id, type, to, height, amount) values (?1, ?2, ?3, ?4, ?5)"#,
                          self.single_balance_tbl_name(&record.coin_id));
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql.as_str())
            .bind(record.account_id.to_string())
            .bind(record.record_type.as_str())
            .bind(record.to.to_string())
            .bind(record.height as i64)
            .bind(record.amount as i64)).await?;
        Ok(())
    }

    async fn drop_unpaid_record(&self, id: u64, coin_id: &CoinTokenId) -> BuckyResult<()> {
        let sql = format!(r#"delete from {}_unpaid where id = ?1"#, self.single_balance_tbl_name(coin_id));
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql.as_str()).bind(id as i64)).await?;
        Ok(())
    }

    async fn get_unpaid_records(&self, account_id: &ObjectId, coin_id: &CoinTokenId) -> BuckyResult<Vec<UnpaidRecord>> {
        let sql = format!(r#"select * from {}_unpaid where account_id=?1 order by height asc"#, self.single_balance_tbl_name(coin_id));
        let mut conn = self.get_conn().await;
        let rows = conn.query_all(sqlx::query(sql.as_str()).bind(account_id.to_string())).await?;
        let mut record_list = Vec::new();
        for row in rows {
            let record = UnpaidRecord {
                id: row.get::<i64, &str>("id") as u64,
                account_id: account_id.clone(),
                to: ObjectId::from_str(row.get("to"))?,
                record_type: row.get("type"),
                height: row.get::<i64, &str>("height") as u64,
                coin_id: coin_id.clone(),
                amount: row.get::<i64, &str>("amount") as u64
            };
            record_list.push(record);
        }
        Ok(record_list)
    }

    async fn get_cycles(&self) -> BuckyResult<Vec<i64>> {
        let mut conn = self.get_conn().await;
        let sql = r#"create table if not exists event_cycles (
            "cycle" INTEGER PRIMARY KEY
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = "select * from event_cycles";
        let rows = conn.query_all(sqlx::query(sql)).await?;
        let mut cycle_list = Vec::new();
        for row in rows {
            cycle_list.push(row.get("cycle"));
        }
        Ok(cycle_list)
    }

    async fn delete_cycle(&self, cycle: i64) -> BuckyResult<()> {
        let sql = "delete form event_cycles where cycle = ?1";
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(cycle)).await?;
        Ok(())
    }

    async fn account_exists(&self, address: &ObjectId) -> BuckyResult<bool> {
        // 是不是有nonce就一定存在这个账户了？先这么判定
        Ok(self.get_nonce(address).await? != 0)
    }

    async fn code(&self, address: &ObjectId) -> BuckyResult<Vec<u8>> {
        let sql = "select code from code where id=?1;";
        let row = self.get_conn().await.query_one(sqlx::query(sql).bind(address.to_string())).await?;
        Ok(row.get("code"))
    }

    async fn storage(&self, address: &ObjectId, index: &H256) -> BuckyResult<H256> {
        let sql = "select value from storage where id=?1 and key=?2;";
        let row = self.get_conn().await.query_one(sqlx::query(sql)
            .bind(address.to_string())
            .bind(hex::encode(index))).await?;
        let ret = H256::from_slice(row.get("value"));
        Ok(ret)
    }

    async fn set_storage(&self, address: &ObjectId, index: &H256, value: H256) -> BuckyResult<()> {
        let sql = "INSERT OR REPLACE INTO storage VALUES (?1, ?2, ?3);";
        self.get_conn().await.execute_sql(sqlx::query(sql)
            .bind(address.to_string())
            .bind(hex::encode(index))
            .bind(value.as_ref())).await?;
        Ok(())
    }

    async fn set_code(&self, address: &ObjectId, code: Vec<u8>) -> BuckyResult<()> {
        let sql = "INSERT OR REPLACE INTO code VALUES (?1, ?2);";
        self.get_conn().await.execute_sql(sqlx::query(sql)
            .bind(address.to_string())
            .bind(code)).await?;
        Ok(())
    }

    async fn reset_storage(&self, address: &ObjectId) -> BuckyResult<()> {
        let sql = "DELETE from storage where id=?1";
        self.get_conn().await.execute_sql(sqlx::query(sql)
            .bind(address.to_string())).await?;
        Ok(())
    }

    async fn remove_storage(&self, address: &ObjectId, index: &H256) -> BuckyResult<()> {
        let sql = "DELETE from storage where id=?1 and key=?2";
        self.get_conn().await.execute_sql(sqlx::query(sql)
            .bind(address.to_string())
            .bind(hex::encode(index))).await?;
        Ok(())
    }

    async fn delete_contract(&self, address: &ObjectId) -> BuckyResult<()> {
        // 移除storage
        self.reset_storage(address).await?;
        // 移除code
        let sql = "DELETE from code where id=?1";
        self.get_conn().await.execute_sql(sqlx::query(sql)
            .bind(address.to_string())).await?;
        Ok(())
    }

    async fn set_log(&self, address: &ObjectId, block_number: i64, topics: &[H256], data: Vec<u8>) -> BuckyResult<()> {
        // topic数量：0, 1, 2, 3
        if topics.len() > 4 {
            return Err(BuckyError::from(BuckyErrorCode::InvalidInput));
        }
        let mut sql = "INSERT into evm_log (address, block".to_owned();

        let mut topic_num: u32 = 0;
        for i in 0..topics.len() {
            sql += format!(", topic{}", i).as_str();
            topic_num += 1;
        }

        sql += ", data) VALUES (?1, ?2";
        for i in 0..topic_num {
            sql += format!(", ?{}", i+3).as_str();
        }
        sql += format!(", ?{})", topic_num+3).as_str();

        let mut query = sqlx::query(sql.as_str())
            .bind(address.to_string())
            .bind(block_number);
        for i in 0..topics.len() {
            query = query.bind(hex::encode(topics[i]));
        }
        query = query.bind(&data);
        self.get_conn().await.execute_sql(query).await?;
        Ok(())
    }

    async fn get_log(&self, address: &ObjectId, from: i64, to: i64, topics: &[Option<H256>]) -> BuckyResult<Vec<(Vec<H256>, Vec<u8>)>> {
        let mut sql = "select * from evm_log where address = ?1".to_owned();
        let mut topic_num:u32 = 0;
        for i in 0..topics.len() {
            if topics[i].is_some() {
                sql += format!(" and topic{} = ?{}", i, topic_num+2).as_str();
                topic_num += 1;
            }
        }

        if from > 0 {
            sql += format!(" and block >= ?{}", topic_num + 2).as_str();
        }
        if to > 0 {
            sql += format!(" and block <= ?{}", topic_num + 3).as_str();
        }
        info!("sql {}", &sql);

        let mut query = sqlx::query(&sql)
            .bind(address.to_string());
        for topic in topics {
            if topic.is_some() {
                query = query.bind(hex::encode(topic.as_ref().unwrap()));
            }
        }
        if from > 0 {
            query = query.bind(from);
        }
        if to > 0 {
            query = query.bind(to);
        }

        let rows = self.get_conn().await.query_all(query).await?;
        let mut ret = vec![];

        for row in rows {
            let mut topics = vec![];
            if let Ok(data) = row.try_get::<&str, &str>("topic0") {
                if data.len() > 0 {
                    topics.push(H256::from_slice(hex::decode(data).unwrap().as_slice()));
                }

            }
            if let Ok(data) = row.try_get::<&str, &str>("topic1") {
                if data.len() > 0 {
                    topics.push(H256::from_slice(hex::decode(data).unwrap().as_slice()));
                }

            }
            if let Ok(data) = row.try_get::<&str, &str>("topic2") {
                if data.len() > 0 {
                    topics.push(H256::from_slice(hex::decode(data).unwrap().as_slice()));
                }

            }
            if let Ok(data) = row.try_get::<&str, &str>("topic3") {
                if data.len() > 0 {
                    topics.push(H256::from_slice(hex::decode(data).unwrap().as_slice()));
                }
            }

            let data = row.get("data");

            ret.push((topics, data));
        }

        Ok(ret)
    }

    async fn set_beneficiary(&self, address: &ObjectId, beneficiary: &ObjectId) -> BuckyResult<()> {
        let sql = "INSERT OR REPLACE INTO address_benefi VALUES (?1, ?2);";
        self.get_conn().await.execute_sql(sqlx::query(sql)
            .bind(address.to_string())
            .bind(beneficiary.to_string())).await?;
        Ok(())
    }

    async fn get_beneficiary(&self, address: &ObjectId) -> BuckyResult<ObjectId> {
        let sql = "select benefi from address_benefi where address = ?1";

        let row = self.get_conn().await.query_one(sqlx::query(sql)
            .bind(address.to_string())).await;
        match row {
            Ok(row) => {
                Ok(ObjectId::from_str(row.get("benefi"))?)
            }
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    Ok(address.clone())
                } else {
                    Err(e)
                }
            }
        }


    }

    async fn nft_create(&self, object_id: &ObjectId, desc: &NFTDesc, name: &str, state: &NFTState) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = "select object_id from nft where object_id = ?1 or nft_label = ?2";
        let ret = conn.query_one(sqlx::query(sql)
            .bind(object_id.to_string())
            .bind(desc.nft_label().to_base58())).await;
        if let Err(err) = ret {
            if ERROR_NOT_FOUND == get_meta_err_code(&err)? {
                let sql = "insert into nft (object_id, nft_label, desc ,name, state) values (?1, ?2, ?3, ?4, ?5)";
                conn.execute_sql(sqlx::query(sql)
                    .bind(object_id.to_string())
                    .bind(desc.nft_label().to_base58())
                    .bind(desc.to_vec()?)
                    .bind(name)
                    .bind(state.to_vec()?)).await?;
                Ok(())
            } else {
                Err(err)
            }
        } else {
            Err(meta_err!(ERROR_ALREADY_EXIST))
        }
    }

    async fn nft_set_name(&self, nft_id: &ObjectId, name: &str) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = "update nft set name = ?1 where object_id = ?2";
        conn.execute_sql(sqlx::query(sql).bind(name).bind(nft_id.to_string())).await?;
        Ok(())
    }

    async fn nft_get(&self, object_id: &ObjectId) -> BuckyResult<(NFTDesc, String, NFTState)> {
        let mut conn = self.get_conn().await;
        let sql = "select * from nft where object_id = ?1";
        let ret = conn.query_one(sqlx::query(sql).bind(object_id.to_string())).await?;
        Ok((
            NFTDesc::clone_from_slice(ret.get("desc"))?,
            ret.get("name"),
            NFTState::clone_from_slice(ret.get("state"))?))
    }

    async fn nft_update_state(&self, object_id: &ObjectId, state: &NFTState) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = "update nft set state = ?1 where object_id = ?2";

        conn.execute_sql(sqlx::query(sql).bind(state.to_vec()?).bind(object_id.to_string())).await?;
        Ok(())
    }

    async fn nft_add_apply_buy(&self, nft_id: &ObjectId, buyer_id: &ObjectId, price: u64, coin_id: &CoinTokenId) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;

        let sql = r#"insert into nft_apply_buy (nft_id, buyer_id, price, coin_id) values (?1, ?2, ?3, ?4)
         ON CONFLICT(nft_id, buyer_id) do update set price = ?3, coin_id = ?4"#;
        conn.execute_sql(sqlx::query(sql)
            .bind(nft_id.to_string())
            .bind(buyer_id.to_string())
            .bind(price as i64)
            .bind(coin_id.to_vec()?)).await?;
        Ok(())
    }


    async fn nft_get_apply_buy(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<Option<(u64, CoinTokenId)>> {
        let mut conn = self.get_conn().await;
        let sql = "select * from nft_apply_buy where nft_id = ?1 and buyer_id = ?2";
        match conn.query_one(sqlx::query(sql).bind(nft_id.to_string()).bind(buyer_id.to_string())).await {
            Ok(row) => {
                Ok(Some((
                    row.get::<i64, _>("price") as u64,
                    CoinTokenId::clone_from_slice(row.get("coin_id"))?)))
            },
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }

    }

    async fn nft_get_apply_buy_list(&self, nft_id: &ObjectId, offset: i64, length: i64) -> BuckyResult<Vec<(ObjectId, u64, CoinTokenId)>> {
        let mut conn = self.get_conn().await;

        let sql = "select * from nft_apply_buy where nft_id = ?1 order by id desc limit ?2, ?3";
        let rows = conn.query_all(sqlx::query(sql).bind(nft_id.to_string()).bind(offset).bind(length)).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push((
                ObjectId::from_str(row.get("buyer_id"))?,
                row.get::<i64, _>("price") as u64,
                CoinTokenId::clone_from_slice(row.get("coin_id"))?
                ));
        }
        Ok(list)
    }

    async fn nft_get_apply_buy_count(&self, nft_id: &ObjectId) -> BuckyResult<i64> {
        let mut conn = self.get_conn().await;
        let sql = "select id from nft_apply_buy where nft_id = ?1";
        let rows = conn.query_all(sqlx::query(sql).bind(nft_id.to_string())).await?;
        Ok(rows.len() as i64)
    }

    async fn nft_remove_all_apply_buy(&self, nft_id: &ObjectId) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = "delete from nft_apply_buy where nft_id = ?1";
        conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string())).await?;
        Ok(())
    }

    async fn nft_remove_apply_buy(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = "delete from nft_apply_buy where nft_id = ?1 and buyer_id = ?2";
        conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string()).bind(buyer_id.to_string())).await?;
        Ok(())
    }

    async fn nft_add_bid(&self, nft_id: &ObjectId, buyer_id: &ObjectId, price: u64, coin_id: &CoinTokenId) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;

        let sql = r#"insert into nft_bid (nft_id, buyer_id, price, coin_id) values (?1, ?2, ?3, ?4)
         ON CONFLICT(nft_id, buyer_id) do update set price = ?3, coin_id = ?4"#;
        conn.execute_sql(sqlx::query(sql)
            .bind(nft_id.to_string())
            .bind(buyer_id.to_string())
            .bind(price as i64)
            .bind(coin_id.to_vec()?)).await?;
        Ok(())
    }

    async fn nft_get_bid(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<Option<(u64, CoinTokenId)>> {
        let mut conn = self.get_conn().await;
        let sql = "select * from nft_bid where nft_id = ?1 and buyer_id = ?2";
        match conn.query_one(sqlx::query(sql).bind(nft_id.to_string()).bind(buyer_id.to_string())).await {
            Ok(row) => {
                Ok(Some((
                    row.get::<i64, _>("price") as u64,
                    CoinTokenId::clone_from_slice(row.get("coin_id"))?)))
            },
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }

    }

    async fn nft_get_bid_list(&self, nft_id: &ObjectId, offset: i64, length: i64) -> BuckyResult<Vec<(ObjectId, u64, CoinTokenId)>> {
        let mut conn = self.get_conn().await;

        let sql = "select * from nft_bid where nft_id = ?1 order by id desc limit ?2, ?3";
        let rows = conn.query_all(sqlx::query(sql).bind(nft_id.to_string()).bind(offset).bind(length)).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push((
                ObjectId::from_str(row.get("buyer_id"))?,
                row.get::<i64, _>("price") as u64,
                CoinTokenId::clone_from_slice(row.get("coin_id"))?
            ));
        }
        Ok(list)
    }

    async fn nft_get_bid_count(&self, nft_id: &ObjectId) -> BuckyResult<i64> {
        let mut conn = self.get_conn().await;
        let sql = "select id from nft_bid where nft_id = ?1";
        let rows = conn.query_all(sqlx::query(sql).bind(nft_id.to_string())).await?;
        Ok(rows.len() as i64)
    }

    async fn nft_remove_all_bid(&self, nft_id: &ObjectId) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = "delete from nft_bid where nft_id = ?1";
        conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string())).await?;
        Ok(())
    }

    async fn nft_remove_bid(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<()> {
        let mut conn = self.get_conn().await;
        let sql = "delete from nft_bid where nft_id = ?1 and buyer_id = ?2";
        conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string()).bind(buyer_id.to_string())).await?;
        Ok(())
    }

    //
    // async fn add_service(&self, service_id: &ObjectId, service_status: u8, service: Vec<u8>) -> BuckyResult<()> {
    //     let sql = "insert into services (service_id, service_status, service) values (1?, 2?, 3?)";
    //     let mut conn = self.get_conn().await;
    //     conn.execute_sql(sqlx::query(sql)
    //         .bind(service_id.to_string())
    //         .bind(service_status)
    //         .bind(service)).await?;
    //     Ok(())
    // }
    //
    // async fn update_service_status(&self, service_id: &ObjectId, service_status: u8) -> BuckyResult<()> {
    //     let sql = "update services set service_status = 1? where service_id = 2?";
    //     let mut conn = self.get_conn().await;
    //     conn.execute_sql(sqlx::query(sql)
    //         .bind(service_status)
    //         .bind(service_id.to_string())).await?;
    //     Ok(())
    // }
    //
    // async fn get_service(&self, service_id: &ObjectId) -> BuckyResult<(u8, Vec<u8>)> {
    //     let sql = "select * from services where service_id = 1?";
    //     let mut conn = self.get_conn().await;
    //     let row = conn.query_one(sqlx::query(sql).bind(service_id.to_string())).await?;
    //     let service_status = row.get("service_status");
    //     let service = row.get("service");
    //     Ok((service_status, service))
    // }
    //
    // async fn add_contract(&self, contract_id: &ObjectId, service_id: &ObjectId, buyer_id: &ObjectId, auth_type: u8, contract: Vec<u8>, auth_list: Vec<u8>) -> BuckyResult<()> {
    //     let sql = "insert into contracts (contract_id, service_id, buyer_id, auth_type, contract, auth_list) values (1?, 2?, 3?, 4?, 5?, 6?)";
    //     let mut conn = self.get_conn().await;
    //     conn.execute_sql(sqlx::query(sql)
    //         .bind(contract_id.to_string())
    //         .bind(service_id.to_string())
    //         .bind(buyer_id.to_string())
    //         .bind(auth_type as i16)
    //         .bind(contract)
    //         .bind(auth_list)).await?;
    //     Ok(())
    // }
    //
    // async fn update_contract(&self, contract_id: &ObjectId, auth_type: u8, auth_list: Vec<u8>) -> BuckyResult<()> {
    //     let sql = "update contracts set auth_type = 1? auth_list = 2? where contract_id = 3?";
    //     let mut conn = self.get_conn().await;
    //     conn.execute_sql(sqlx::query(sql)
    //         .bind(auth_type as i16)
    //         .bind(auth_list)
    //         .bind(contract_id.to_string())).await?;
    //     Ok(())
    // }
    //
    // async fn get_contract(&self, contract_id: &ObjectId) -> BuckyResult<(Vec<u8>, u8, Vec<u8>)> {
    //     let sql = "select * from contract_id = 1?";
    //     let mut conn = self.get_conn().await;
    //     let row = conn.query_one(sqlx::query(sql)
    //         .bind(contract_id.to_string())).await?;
    //     let contract = row.get("contract");
    //     let auth_type: i16 = row.get("auth_type");
    //     let auth_list = row.get("auth_list");
    //     Ok((contract, auth_type as u8, auth_list))
    // }
    //
    // async fn get_contract_by_buyer(&self, service_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<(Vec<u8>, u8, Vec<u8>)> {
    //     let sql = "select * from service_id = 1? and buyer_id = 2?";
    //     let mut conn = self.get_conn().await;
    //     let row = conn.query_one(sqlx::query(sql)
    //         .bind(service_id.to_string())
    //         .bind(buyer_id.to_string())).await?;
    //     let contract = row.get("contract");
    //     let auth_type: i16 = row.get("auth_type");
    //     let auth_list = row.get("auth_list");
    //     Ok((contract, auth_type as u8, auth_list))
    // }
}

pub struct SqlStorage {
    path: PathBuf,
    locker: Mutex<()>,
    conn_pool: sqlx::SqlitePool
}

// impl Drop for SqlState {
//     fn drop(&mut self) {
//         info!("drop db connection");
//     }
// }

#[async_trait]
impl Storage for SqlStorage {
    fn path(&self) -> &Path {
        self.path.as_path()
    }

    async fn create_state(&self, _read_only: bool) -> StateRef {
        let _locker = self.get_locker().await;
        let conn = self.conn_pool.acquire().await;
        if let Err(e) = &conn {
            let msg = format!("{:?}", e);
            info!("{}", msg);
        }
        let conn = conn.unwrap();
        SqlState::new(conn)
    }

    async fn state_hash(&self) -> BuckyResult<StateHash> {
        let _locker = self.get_locker().await;
        static SQLITE_HEADER_SIZE: usize = 100;
        let content = std::fs::read(self.path()).map_err(|err| {
            error!("read file {} fail, err {}", self.path.display(), err);
            crate::meta_err!(ERROR_NOT_FOUND)})?;
        let mut hasher = Sha256::new();
        hasher.input(&content[SQLITE_HEADER_SIZE..]);
        Ok(HashValue::from(hasher.result()))
    }

    async fn get_locker(&self) -> MutexGuard<'_, ()> {
        self.locker.lock().await
    }
}

pub fn new_sql_storage(path: &Path) -> StorageRef {
    let mut options= if path == storage_in_mem_path() {
        MetaConnectionOptions::new()
            .journal_mode(SqliteJournalMode::Memory)
    } else {
        MetaConnectionOptions::from_str(format!("sqlite://{}", path.to_str().unwrap()).as_str()).unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory)
    };
    options
        .log_statements(LevelFilter::Off)
        .log_slow_statements(LevelFilter::Off, Duration::new(10, 0));

    Arc::new(Box::new(SqlStorage {
        path: PathBuf::from(path.to_str().unwrap()),
        locker: Default::default(),
        conn_pool: sqlx::Pool::connect_lazy_with(options)
    }))
}

#[cfg(test)]
pub mod sql_storage_tests {
    use crate::state_storage::{Storage, storage_in_mem_path, StateRef, StorageRef, SqlStorage, SqlState, new_sql_storage};
    use cyfs_base_meta::{StateHash, GenesisCoinConfig};
    use cyfs_base::{BuckyResult, NameInfo, NameRecord, NameLink, HashValue};
    use std::path::Path;
    use std::collections::HashMap;
    use async_trait::async_trait;
    use sqlx::{ConnectOptions};
    use async_std::sync::{Arc, MutexGuard, Mutex};
    use crate::{MetaConnectionOptions, State};
    use log::LevelFilter;
    use std::time::Duration;
    use sqlx::sqlite::SqliteJournalMode;

    pub struct TestStorage {
        storage: SqlStorage,
        state: StateRef,
        locker: Mutex<()>,
    }

    unsafe impl Send for TestStorage {

    }

    #[async_trait]
    impl Storage for TestStorage {
        fn path(&self) -> &Path {
            self.storage.path.as_path()
        }

        async fn create_state(&self, read_only: bool) -> StateRef {
            self.state.clone()
        }

        async fn state_hash(&self) -> BuckyResult<StateHash> {
            Ok(HashValue::default())
        }

        async fn get_locker(&self) -> MutexGuard<'_, ()> {
            self.locker.lock().await
        }
    }

    pub async fn create_test_storage() -> StorageRef {
        let mut options = MetaConnectionOptions::new()
            .journal_mode(SqliteJournalMode::Memory);
        options.log_statements(LevelFilter::Off)
            .log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
        let pool = sqlx::SqlitePool::connect_lazy_with(options);
        let state = SqlState::new(pool.acquire().await.unwrap());
        state.init_genesis(&vec![GenesisCoinConfig {
            coin_id: 0,
            pre_balance: vec![]
        }]).await.unwrap();
        state.init().await.unwrap();
        Arc::new(Box::new(TestStorage {
            storage: SqlStorage {
                path: storage_in_mem_path().to_path_buf(),
                locker: Default::default(),
                conn_pool: pool
            },
            state,
            locker: Default::default()
        }))
    }

    pub async fn create_state() -> StateRef {
        let state = new_sql_storage(storage_in_mem_path()).create_state(false).await;
        state.init_genesis(&vec![GenesisCoinConfig {
            coin_id: 0,
            pre_balance: vec![]
        }]).await.unwrap();
        state.init().await.unwrap();
        state
    }

    #[test]
    fn test_transaction() {
        async_std::task::block_on(async {
            let state = create_state().await;
            let ret = state.being_transaction().await;
            assert!(ret.is_ok());

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.get_name_info("test").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_some());

            let ret = state.create_name_info("test2", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.commit().await;
            assert!(ret.is_ok());

            let ret = state.get_name_info("test").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_some());

            let ret = state.get_name_info("test2").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_some());

            let ret = state.being_transaction().await;
            assert!(ret.is_ok());

            let ret = state.create_name_info("test3", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.get_name_info("test3").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_some());

            let ret = state.create_name_info("test4", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.rollback().await;
            assert!(ret.is_ok());

            let ret = state.get_name_info("test3").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_none());

            let ret = state.get_name_info("test4").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_none());

            let ret = state.being_transaction().await;
            assert!(ret.is_ok());

            let ret = state.create_name_info("test3", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.get_name_info("test3").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_some());

            let ret = state.create_name_info("test4", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.commit().await;
            assert!(ret.is_ok());

            let ret = state.get_name_info("test3").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_some());

            let ret = state.get_name_info("test4").await;
            assert!(ret.is_ok());
            assert!(ret.as_ref().unwrap().is_some());
        });
    }
}
