use std::path::{Path, PathBuf};
use log::*;
use sha2::{Sha256, Digest};
use async_trait::async_trait;

use cyfs_base::*;
use cyfs_base_meta::*;
use crate::state_storage::*;
use rusqlite::{Connection, NO_PARAMS};
use std::sync::{Arc, Mutex, MutexGuard};
use std::str::FromStr;

pub struct SqliteState {
    conn: Arc<Mutex<Connection>>
}

const PEERID_LENGTH: u32 = 32;

impl SqliteState {
    pub fn new (conn: Connection) -> StateRef {
        StateRef::new(Box::new(SqliteState {
            conn: Arc::new(Mutex::new(conn))
        }))
    }

    fn map_sql_err(err: rusqlite::Error) -> BuckyError {
        log::error!("sqlite error: {:?}", err);
        match err {
            rusqlite::Error::QueryReturnedNoRows => BuckyError::from(ERROR_NOT_FOUND),
            _ => BuckyError::from(ERROR_EXCEPTION)
        }
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

    fn init_balance_tbl(&self, ctid: &CoinTokenId) -> BuckyResult<()> {
        {
            let sql = format!("CREATE TABLE IF NOT EXISTS \"{}\"
                (\"id\" CHAR({}) PRIMARY KEY NOT NULL UNIQUE,
                \"balance\" INTEGER NOT NULL);", self.single_balance_tbl_name(ctid), PEERID_LENGTH * 2);
            let conn = self.get_conn();
            conn.execute(&sql, NO_PARAMS).map_err(Self::map_sql_err)?;
        }

        {
            let sql = format!("CREATE TABLE IF NOT EXISTS \"{}\"
                (\"id\" CHAR({}) PRIMARY KEY NOT NULL UNIQUE,
                \"left_balance\" INTEGER NOT NULL,
                \"right_balance\" INTEGER NOT NULL,
                \"deviation\" INTEGER NOT NULL,
                \"seq\" INTEGER NOT NULL);",
                self.union_balance_tbl_name(ctid), PEERID_LENGTH * 2);
            let conn = self.get_conn();
            conn.execute(&sql, NO_PARAMS).map_err(Self::map_sql_err)?;
        }


        Ok(())
    }

    fn init_obj_desc_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS all_descs (
	                "obj_id" char(45) NOT NULL UNIQUE,
	                "desc" BLOB NOT NULL,
	                "update_time" INTEGER,
	                PRIMARY KEY("obj_id"));"#;

        let conn = self.get_conn();
        conn.execute(&sql, NO_PARAMS).map_err(Self::map_sql_err)?;
        return Ok(());
    }

    fn init_desc_rent_state_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS desc_extra (
        "obj_id" char(45) PRIMARY KEY NOT NULL UNIQUE,
        "rent_arrears" INTEGER,
        "rent_arrears_count" INTEGER,
        "rent_value" INTEGER,
        "coin_id" INTEGER,
        "data_len" INTEGER,
        "other_charge_balance" INTEGER);"#;

        let conn = self.get_conn();
        conn.execute(&sql, rusqlite::NO_PARAMS).map_err(Self::map_sql_err)?;
        return Ok(());
    }

    fn get_conn(&self) -> MutexGuard<Connection> {
        self.conn.lock().unwrap()
    }

    fn init_name_info_table(&self) -> BuckyResult<()> {
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
        let conn = self.get_conn();
        conn.execute(&sql, NO_PARAMS).map_err(Self::map_sql_err)?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS owner_index ON all_names (owner)"#;
        conn.execute(&index_sql, NO_PARAMS).map_err(Self::map_sql_err)?;

        return Ok(());
    }

    fn get_cycle_event_table_name(&self, cycle: i64) -> String {
        format!("cycle_event_{}", cycle)
    }

    fn init_cycle_event_table(&self, cycle: i64) -> BuckyResult<()> {
        let table_name = self.get_cycle_event_table_name(cycle);
        let sql = format!(r#"CREATE TABLE IF NOT EXISTS {} (
            "key"	char(45) NOT NULL,
            "height" INTEGER NOT NULL,
            "real_key" TEXT NOT NULL,
            "start_height" INTEGER NOT NULL,
            "param"	BLOB NOT NULL,
            PRIMARY KEY("key")
        )"#, table_name);

        let conn = self.get_conn();
        conn.execute(sql.as_str(), NO_PARAMS).map_err(Self::map_sql_err)?;

        let index_sql = format!(r#"CREATE INDEX IF NOT EXISTS height_index ON {} (height)"#, table_name);
        conn.execute(&index_sql, NO_PARAMS).map_err(Self::map_sql_err)?;

        return Ok(());
    }

    fn init_event_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "event" (
            "type"	INTEGER NOT NULL,
            "key"	TEXT NOT NULL,
            "param"	BLOB NOT NULL,
            "height"	INTEGER NOT NULL,
            PRIMARY KEY("type","key","height")
        )"#;
        let conn = self.get_conn();
        conn.execute(&sql, NO_PARAMS).map_err(Self::map_sql_err)?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS height_index ON event (height)"#;
        conn.execute(&index_sql, NO_PARAMS).map_err(Self::map_sql_err)?;

        return Ok(());
    }

    fn init_once_event_table(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "once_event" (
            "key"	TEXT NOT NULL,
            "height"	INTEGER NOT NULL,
            "param"	BLOB NOT NULL,
            PRIMARY KEY("key")
        )"#;
        let conn = self.get_conn();
        conn.execute(&sql, NO_PARAMS).map_err(Self::map_sql_err)?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS height_index ON once_event (height)"#;
        conn.execute(&index_sql, NO_PARAMS).map_err(Self::map_sql_err)?;

        return Ok(());
    }

    fn account_tbl_name(&self) -> &'static str {
        static NONCE_TBL_NAME: &str = "account";
        NONCE_TBL_NAME
    }

    fn init_account_tbl(&self) -> BuckyResult<()> {
        let sql = format!("CREATE TABLE IF NOT EXISTS \"{}\"
            (\"id\" CHAR({}) PRIMARY KEY NOT NULL UNIQUE,
            \"nonce\" INTEGER NOT NULL);", self.account_tbl_name(), PEERID_LENGTH * 2);
        let conn = self.get_conn();
        conn.execute(&sql, NO_PARAMS).map_err(Self::map_sql_err)?;
        Ok(())
    }

    fn init_config_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "config" (
            "key"	TEXT NOT NULL,
            "value"	TEXT NOT NULL,
            PRIMARY KEY("key")
        )"#;

        let conn = self.get_conn();
        conn.execute(&sql, NO_PARAMS).map_err(Self::map_sql_err)?;
        return Ok(());
    }

    fn get_config(&self, key: &str, default: Option<String>) -> BuckyResult<String> {
        let sql = "select value from config where key=?1";
        let conn = self.get_conn();
        match conn.query_row(sql, rusqlite::params![key], |row| {
            let value:String = row.get(0)?;
            Ok(value)
        }) {
            Ok(value) => {Ok(value)},
            Err(e) => {
                if let rusqlite::Error::QueryReturnedNoRows = e {
                    if default.is_some() {
                        return Ok(default.unwrap());
                    }
                }
                Err(BuckyError::from(ERROR_EXCEPTION))
            },
        }
    }
}

#[async_trait]
impl State for SqliteState {
    async fn being_transaction(&self) -> BuckyResult<()> {
        let conn = self.get_conn();
        conn.execute("BEGIN TRANSACTION;", rusqlite::NO_PARAMS).map_err(Self::map_sql_err)?;
        Ok(())
    }

    async fn rollback(&self) -> BuckyResult<()> {
        let conn = self.get_conn();
        conn.execute("ROLLBACK;", rusqlite::NO_PARAMS).map_err(Self::map_sql_err)?;
        Ok(())
    }

    async fn commit(&self) -> BuckyResult<()> {
        let conn = self.get_conn();
        conn.execute("COMMIT;", rusqlite::NO_PARAMS).map_err(Self::map_sql_err)?;
        Ok(())
    }

    async fn init(&self) -> BuckyResult<()> {
        self.init_event_table()?;
        self.init_desc_rent_state_table()?;
        self.init_once_event_table()?;
        Ok(())
    }

    async fn create_cycle_event_table(&self, cycle: i64) -> BuckyResult<()> {
        self.init_cycle_event_table(cycle)
    }

    async fn config_get(&self, key: &str, default: &str) -> BuckyResult<String>
    {
        let sql = "select value from config where key=?1";
        let conn = self.get_conn();
        match conn.query_row(sql, rusqlite::params![key], |row| {
            let value:String = row.get(0)?;
            Ok(value)
        }) {
            Ok(value) => {Ok(value)},
            Err(e) => {
                if let rusqlite::Error::QueryReturnedNoRows = e {
                    return Ok(default.to_owned());
                }
                Err(BuckyError::from(ERROR_EXCEPTION))
            },
        }
    }

    async fn config_set(&self, key: &str, value: &str) -> BuckyResult<()> {
        let sql = "insert into config values (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2";
        let conn = self.get_conn();
        if let Err(e) = conn.execute(sql, rusqlite::params![key, value]) {
            error!("set config err {}, key {}, value {}", e, key, value);
            return Err(BuckyError::from(ERROR_EXCEPTION));
        }
        Ok(())
    }

    async fn init_genesis(&self, config: &GenesisConfig) -> BuckyResult<()> {
        // init account
        self.init_account_tbl()?;

        // init coins
        for coin in &config.coins {
            self.init_balance_tbl(&CoinTokenId::Coin(coin.coin_id))?;
            for (account, balance) in &coin.pre_balance {
                self.modify_balance(&CoinTokenId::Coin(coin.coin_id), account, *balance).await?;
            }
        }

        // init obj-desc
        self.init_obj_desc_table()?;

        // init name-info
        self.init_name_info_table()?;

        self.init_config_tbl()?;
        self.init_event_table()?;
        self.init_desc_rent_state_table()?;
        self.init_once_event_table()?;
        Ok(())
    }

    async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<i64> {
        let sql = format!("SELECT nonce FROM {} WHERE id=?1", self.account_tbl_name());
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &sql,
            rusqlite::params![account.to_hex()?],
            |row| {
                let nonce: i64 = row.get(0)?;
                Ok(nonce)
            });
        if let Err(err) = query_result {
            if let rusqlite::Error::QueryReturnedNoRows = err {
                Ok(0)
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        } else {
            Ok(query_result.unwrap())
        }
    }

    async fn inc_nonce(&self, account: &ObjectId) -> BuckyResult<i64> {
        let nonce;
        let qeury_sql = format!("SELECT nonce FROM {} WHERE id=?1", self.account_tbl_name());
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &qeury_sql,
            rusqlite::params![account.to_hex()?],
            |row| {
                let nonce: i64 = row.get(0)?;
                Ok(nonce)
            });
        if let Err(err) = query_result {
            if let rusqlite::Error::QueryReturnedNoRows = err {
                nonce = 1;
                let insert_sql = format!("INSERT INTO {} (id, nonce) VALUES (?1, ?2)", self.account_tbl_name());
                conn.execute(&insert_sql, rusqlite::params![account.to_hex()?, 1]).map_err(Self::map_sql_err)?;
            } else {
                return Err(BuckyError::from(ERROR_EXCEPTION));
            }
        } else {
            nonce = query_result.unwrap() + 1;
            let update_sql = format!("UPDATE {} SET nonce=?1 WHERE id=?2", self.account_tbl_name());
            conn.execute(&update_sql, rusqlite::params![nonce, account.to_hex()?]).map_err(Self::map_sql_err)?;
        }

        Ok(nonce)
    }

    async fn get_account_permission(&self, _account: &ObjectId) -> BuckyResult<u32> {
        unimplemented!()
    }

    async fn get_balance(&self, account: &ObjectId, ctid: &CoinTokenId) -> BuckyResult<i64> {
        let sql = format!("SELECT balance FROM {} WHERE id=?1", self.single_balance_tbl_name(ctid));
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &sql,
            rusqlite::params![account.to_hex()?],
            |row| {
                let balance: i64 = row.get(0)?;
                Ok(balance)
            });
        if let Err(err) = query_result {
            if let rusqlite::Error::QueryReturnedNoRows = err {
                Ok(0)
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        } else {
            Ok(query_result.unwrap())
        }
    }

    async fn modify_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()> {
        let sql = format!("REPLACE INTO {} (balance, id) VALUES (?1, ?2)", self.single_balance_tbl_name(ctid));
        let conn = self.get_conn();
        let changed = conn.execute(&sql, rusqlite::params![v, account.to_hex()?]).map_err(Self::map_sql_err)?;
        if changed != 1 {
            Err(BuckyError::from(ERROR_EXCEPTION))
        } else {
            Ok(())
        }
    }


    async fn inc_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()> {
        if v == 0 {
            return Ok(());
        }
        let update_sql = format!("UPDATE {} SET balance=balance+?1 WHERE id=?2", self.single_balance_tbl_name(ctid));
        let conn = self.get_conn();
        let changed = conn.execute(&update_sql, rusqlite::params![v, account.to_hex()?]).map_err(Self::map_sql_err)?;
        if changed != 1 {
            let insert_sql = format!("INSERT INTO {} (balance, id) VALUES (?1, ?2)", self.single_balance_tbl_name(ctid));
            let changed = conn.execute(&insert_sql, rusqlite::params![v, account.to_hex()?]).map_err(Self::map_sql_err)?;
            if changed != 1 {
                return Err(BuckyError::from(ERROR_EXCEPTION));
            }
        }
        Ok(())
    }

    async fn dec_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()> {
        if v == 0 {
            return Ok(());
        }
        let sql = format!("UPDATE {} SET balance=balance-?1 WHERE id=?2 AND balance>=?1", self.single_balance_tbl_name(ctid));
        let conn = self.get_conn();
        let changed = conn.execute(&sql, rusqlite::params![v, account.to_hex()?]).map_err(Self::map_sql_err)?;
        if changed != 1 {
            warn!("dec {} balance {} fail", account, v);
            Err(BuckyError::from(ERROR_NO_ENOUGH_BALANCE))
        } else {
            Ok(())
        }
    }

    async fn issue_token(
        &self,
        _to: &ObjectId,
        _v: u64,
        token_id: &ObjectId) -> BuckyResult<()> {
        self.init_balance_tbl(&CoinTokenId::Token(*token_id))
    }

    async fn get_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId) -> BuckyResult<UnionBalance> {
        let total = self.get_balance(union, ctid).await?;
        let sql = format!("SELECT left_balance, right_balance, deviation FROM {} WHERE id=?1", self.union_balance_tbl_name(ctid));
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &sql,
            rusqlite::params![union.to_hex()?],
            |row| {
                let left: i64 = row.get(0)?;
                let right: i64 = row.get(1)?;
                let deviation: i64 = row.get(2)?;
                Ok(UnionBalance{
                    total,
                    left,
                    right,
                    deviation
                })
            });
        if let Err(err) = query_result {
            warn!("query union balance err {}", err);
            if let rusqlite::Error::QueryReturnedNoRows = err {
                Ok(UnionBalance::default())
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        } else {
            Ok(query_result.unwrap())
        }
    }

    async fn get_union_deviation_seq(&self, ctid: &CoinTokenId, union: &ObjectId) -> BuckyResult<i64> {
        let sql = format!("SELECT seq FROM {} WHERE id=?1", self.union_balance_tbl_name(ctid));
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &sql,
            rusqlite::params![union.to_hex()?],
            |row| {
                let seq: i64 = row.get(0)?;
                Ok(seq)
            } );
        if let Err(e) = query_result {
            error!("get deviation seq err {}", e);
            Err(BuckyError::from(ERROR_EXCEPTION))
        } else {
            Ok(query_result.unwrap())
        }
    }

    async fn update_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId, balance: &UnionBalance) -> BuckyResult<()> {
        let sql = format!("UPDATE {} SET left_balance=?1, right_balance=?2, deviation=?3 WHERE id=?4", self.union_balance_tbl_name(ctid));

        let conn = self.get_conn();
        let changed = conn.execute(&sql, rusqlite::params![balance.left, balance.right, balance.deviation, union.to_hex()?]).map_err(Self::map_sql_err)?;
        if changed != 1 {
            Err(BuckyError::from(ERROR_EXCEPTION))
        } else {
            Ok(())
        }
    }

    async fn deposit_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId, from: PeerOfUnion, v: i64) -> BuckyResult<()> {
        let pre_name = self.union_balance_col_pre_name(&from);
        let update_sql = format!("UPDATE {} SET {}_balance={}_balance+?1 WHERE id=?2", self.union_balance_tbl_name(ctid), pre_name, pre_name);
        let conn = self.get_conn();
        let changed = conn.execute(&update_sql, rusqlite::params![v, union.to_hex()?]).map_err(Self::map_sql_err)?;
        if changed != 1 {
            let insert_sql = match from {
                PeerOfUnion::Left => format!("INSERT INTO {} (id, left_balance, right_balance, deviation, seq) VALUES (?1, ?2, 0, 0, -1)", self.union_balance_tbl_name(ctid)),
                PeerOfUnion::Right => format!("INSERT INTO {} (id, right_balance, left_balance, deviation, seq) VALUES (?1, ?2, 0, 0, -1)", self.union_balance_tbl_name(ctid))
            };
            let changed = conn.execute(&insert_sql, rusqlite::params![union.to_hex()?, v]).map_err(Self::map_sql_err)?;
            if changed != 1 {
                return Err(BuckyError::from(ERROR_EXCEPTION));
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
                    Err(BuckyError::from(ERROR_NO_ENOUGH_BALANCE))
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
                    Err(BuckyError::from(ERROR_NO_ENOUGH_BALANCE))
                }
            }
        };
    }

    async fn update_union_deviation(&self, ctid: &CoinTokenId, union: &ObjectId, deviation: i64, seq: i64) -> BuckyResult<()> {
        let old_seq = self.get_union_deviation_seq(ctid, union).await?;
        if old_seq >= seq {
            return Err(BuckyError::from(ERROR_ACCESS_DENIED));
        }

        let balance = self.get_union_balance(ctid, union).await?;
        if balance.left + deviation < 0 || balance.right - deviation < 0 {
            return Err(BuckyError::from(ERROR_NO_ENOUGH_BALANCE));
        }
        let update_sql = format!("UPDATE {} SET deviation=?1, seq=?2 WHERE id=?3", self.union_balance_tbl_name(ctid));
        let conn = self.get_conn();
        let changed = conn.execute(&update_sql, rusqlite::params![deviation, seq, union.to_hex()?]).map_err(Self::map_sql_err)?;
        if changed != 1 {
            return Err(BuckyError::from(ERROR_NOT_FOUND));
        }
        Ok(())
    }


    async fn get_desc_extra(&self, id: &ObjectId) -> BuckyResult<DescExtra> {
        let sql = "SELECT rent_arrears, rent_value, coin_id, data_len, other_charge_balance, rent_arrears_count from desc_extra where obj_id=?1";
        let conn = self.get_conn();
        conn.query_row(sql, rusqlite::params![id.to_string()], |row| {
            Ok(DescExtra {
                obj_id: id.clone(),
                rent_arrears: row.get(0)?,
                rent_value: row.get(1)?,
                coin_id: row.get(2)?,
                data_len: row.get(3)?,
                other_charge_balance: row.get(4)?,
                rent_arrears_count: row.get(5)?,
            })
        }).or_else(|e| {
            if QueryReturnedNoRows == e {
                Err(BuckyError::from(ERROR_NOT_FOUND))
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        })
    }

    async fn add_or_update_desc_extra(&self, state: &DescExtra) -> BuckyResult<()> {
        let sql = r#"INSERT INTO desc_extra (obj_id, rent_arrears, rent_value, coin_id, data_len, other_charge_balance, rent_arrears_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(obj_id) DO UPDATE SET rent_arrears=?2, rent_value=?3, coin_id=?4, data_len=?5, other_charge_balance=?6, rent_arrears_count=?7"#;
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![state.obj_id.to_string(), state.rent_arrears, state.rent_value, state.coin_id, state.data_len, state.other_charge_balance, state.rent_arrears_count])
            .map_err(Self::map_sql_err)?;
        Ok(())
    }

    async fn add_or_update_desc_rent_state(&self, state: &DescExtra) -> BuckyResult<()> {
        let sql = r#"INSERT INTO desc_extra (obj_id, rent_arrears, rent_value, coin_id, data_len, other_charge_balance, rent_arrears_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(obj_id) DO UPDATE SET rent_arrears=?2, rent_value=?3, coin_id=?4, data_len=?5, rent_arrears_count=?7"#;
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![state.obj_id.to_string(), state.rent_arrears, state.rent_value, state.coin_id, state.data_len, state.other_charge_balance, state.rent_arrears_count])
            .map_err(Self::map_sql_err)?;
        Ok(())
    }

    async fn add_or_update_desc_other_charge_balance(&self, state: &DescExtra) -> BuckyResult<()> {
        let sql = r#"INSERT INTO desc_extra (obj_id, rent_arrears, rent_value, coin_id, data_len, other_charge_balance, rent_arrears_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(obj_id) DO UPDATE SET other_charge_balance=?6"#;
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![state.obj_id.to_string(), state.rent_arrears, state.rent_value, state.coin_id, state.data_len, state.other_charge_balance, state.rent_arrears_count])
            .map_err(Self::map_sql_err)?;
        Ok(())
    }

    async fn update_desc_extra(&self, state: &DescExtra) -> BuckyResult<()> {
        let sql = "UPDATE desc_extra set rent_arrears=?1, rent_value=?2, coin_id=?3, data_len=?4, other_charge_balance=?5 where obj_id=?6";
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![state.rent_arrears, state.rent_value, state.coin_id, state.data_len, state.other_charge_balance, state.obj_id.to_string()])
            .map_err(Self::map_sql_err)?;
        Ok(())
    }

    async fn drop_desc_extra(&self, obj_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from desc_extra where obj_id=?1";
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![obj_id.to_string()])
            .map_err(Self::map_sql_err)?;
        Ok(())
    }

    async fn get_name_extra(&self, id: &str) -> BuckyResult<NameExtra> {
        let sql = "SELECT rent_arrears, rent_value, owner, coin_id, buy_price, buy_coin_id, rent_arrears_count from all_names where name_id=?1";
        let conn = self.get_conn();
        conn.query_row(sql, rusqlite::params![id.to_string()], |row| {
            let ower_str: String = row.get(2)?;
            let coin_id: u8 = row.get(3)?;
            Ok(NameExtra {
                name_id: id.to_string(),
                rent_arrears: row.get(0)?,
                rent_value: row.get(1)?,
                coin_id,
                owner: ObjectId::from_str(ower_str.as_str()).or_else(|_| Err(rusqlite::Error::SqliteSingleThreadedMode))?,
                buy_price: row.get(4)?,
                buy_coin_id: row.get(5)?,
                rent_arrears_count: row.get(6)?,
            })
        }).or_else(|e| {
            error!("sqlite error: {:?}", e);
            if QueryReturnedNoRows == e {
                Err(BuckyError::from(ERROR_NOT_FOUND))
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        })
    }

    async fn add_or_update_name_extra(&self, state: &NameExtra) -> BuckyResult<()> {
        let sql = "UPDATE all_names set rent_arrears=?1, rent_arrears_count=?6, rent_value=?2, buy_price=?3, buy_coin_id=?5 where name_id=?4";
        let conn = self.get_conn();
        let num = conn.execute(sql, rusqlite::params![state.rent_arrears, state.rent_value, state.buy_price
        , state.name_id.to_string(), state.buy_coin_id, state.rent_arrears_count])
            .or_else(|_| Err(BuckyError::from(ERROR_EXCEPTION)))?;
        if num == 0 {
            let sql = "INSERT INTO all_names (name_id, owner, rent_arrears, rent_arrears_count, rent_value, buy_price, buy_coin_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
            conn.execute(sql, rusqlite::params![state.name_id.to_string(), state.owner.to_string(), state.rent_arrears
            , state.rent_arrears_count, state.rent_value, state.buy_price, state.buy_coin_id])
                .map_err(Self::map_sql_err)?;
        }
        Ok(())
    }

    async fn add_or_update_name_rent_state(&self, state: &NameExtra) -> BuckyResult<()> {
        let sql = "UPDATE all_names set rent_arrears=?, rent_arrears_count=?, rent_value=? where name_id=?";
        let conn = self.get_conn();
        let num = conn.execute(sql, rusqlite::params![state.rent_arrears, state.rent_arrears_count, state.rent_value, state.name_id.to_string()])
            .or_else(|_| Err(BuckyError::from(ERROR_EXCEPTION)))?;
        if num == 0 {
            let sql = "INSERT INTO all_names (name_id, owner, rent_arrears, rent_arrears_count, rent_value, buy_price, buy_coin_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
            conn.execute(sql, rusqlite::params![state.name_id.to_string(), state.owner.to_string(), state.rent_arrears
            , state.rent_arrears_count, state.rent_value, state.buy_price, state.buy_coin_id])
                .map_err(Self::map_sql_err)?;
        }
        Ok(())
    }

    async fn add_or_update_name_buy_price(&self, state: &NameExtra) -> BuckyResult<()> {
        let sql = "UPDATE all_names set buy_price=? where name_id=?";
        let conn = self.get_conn();
        let num = conn.execute(sql, rusqlite::params![state.buy_price, state.name_id.to_string()])
            .or_else(|_| Err(BuckyError::from(ERROR_EXCEPTION)))?;
        if num == 0 {
            let sql = "INSERT INTO all_names (name_id, owner, rent_arrears, rent_arrears_count, rent_value, buy_price, buy_coin_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
            conn.execute(sql, rusqlite::params![state.name_id.to_string(), state.owner.to_string(), state.rent_arrears
            , state.rent_arrears_count, state.rent_value, state.buy_price, state.buy_coin_id])
                .map_err(Self::map_sql_err)?;
        }
        Ok(())
    }

    async fn create_name_info(&self,name:&str,info:&NameInfo)->BuckyResult<()> {
        let sql = "SELECT name_state FROM all_names WHERE name_id=?1";
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &sql,
            rusqlite::params![name],
            |_| {
                return Ok(());
            });

        return if let Err(err) = query_result {
            if let rusqlite::Error::QueryReturnedNoRows = err {
                let sql = r#"INSERT INTO all_names (name_id,name_info,name_state,owner,coin_id,rent_arrears
        ,rent_value,buy_coin_id,buy_price) VALUES (?1,?2,?3,?4,0, 0, 0, 0, 0)"#;
                let name_info_data_raw = info.to_vec();
                if name_info_data_raw.is_err() {
                    log::error!("serialize name info error!");
                    return Err(BuckyError::from(ERROR_PARAM_ERROR));
                }
                let name_info_data_raw = name_info_data_raw.unwrap();
                // Create的时候，Name一定是Auction1状态
                let owner = if info.owner.is_some() {
                    info.owner.unwrap().to_string()
                } else {
                    String::from("")
                };
                let insert_result = conn.execute(&sql, rusqlite::params![name,name_info_data_raw,NameState::Auction as i32,owner]).map_err(Self::map_sql_err)?;

                if insert_result != 1 {
                    return Err(BuckyError::from(ERROR_ALREADY_EXIST));
                }
                Ok(())
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        } else {
            Err(BuckyError::from(ERROR_ALREADY_EXIST))
        }
    }

    async fn get_name_info(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>> {
        let sql = "SELECT name_info,name_state FROM all_names WHERE name_id=?1";
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &sql,
            rusqlite::params![name],
            |row| {
                let name_info_data: Vec<u8> = row.get(0)?;
                let name_state: i32 = row.get(1)?;
                let state = NameState::from(name_state);
                let name_info_result : NameInfo = NameInfo::clone_from_slice(name_info_data.as_slice()).map_err(|e| {
                    error!("get_name_info error:{} {}", e.code() as u32, e.msg());
                    rusqlite::Error::QueryReturnedNoRows
                })?;
                return Ok(Some((name_info_result,state)));
            });

        return if let Err(err) = query_result {
            if let rusqlite::Error::QueryReturnedNoRows = err {
                Ok(None)
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        } else {
            Ok(query_result.unwrap())
        }

    }

    async fn get_owned_names(&self, owner: &ObjectId)->BuckyResult<Vec<String>> {
        let sql = "select name_id from all_names where owner=?1";
        let conn = self.get_conn();
        let mut stmt = conn.prepare(sql).map_err(|e| {
            error!("prepare get event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        let rows = stmt.query_map(rusqlite::params![owner.to_string()], |row|{
            let name: String = row.get(0)?;
            Ok(name)
        }).map_err(|e|{
            error!("query get_owner_names err {}, owner {}", e, owner.to_string());
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        let mut ret = vec![];
        for row in rows {
            if let Ok(name) = row {
                ret.push(name);
            }
        }
        Ok(ret)
    }

    async fn get_name_state(&self, name: &str) -> BuckyResult<NameState> {
        let sql = "select name_state from all_names where name_id=?";
        let conn = self.get_conn();
        conn.query_row(
            &sql,
            rusqlite::params![name],
            |row| {
                let name_state: i32 = row.get(0)?;
                let state = NameState::from(name_state);
                return Ok(state);
            }).map_err(Self::map_sql_err)
    }

    async fn update_name_info(&self, name: &str, info: &NameInfo) -> BuckyResult<()> {
        let sql = "UPDATE all_names SET name_info=?1, owner=?3 WHERE name_id=?2";
        let name_info_data_raw = info.to_vec();
        if name_info_data_raw.is_err() {
            log::error!("serialize name info error!");
            return Err(BuckyError::from(ERROR_PARAM_ERROR));
        }
        let name_info_data_raw = name_info_data_raw.unwrap();
        let mut owner = "".to_owned();
        if info.owner.is_some() {
            owner = info.owner.unwrap().to_string();
        }

        let conn = self.get_conn();
        let changed = conn.execute(&sql, rusqlite::params![name_info_data_raw,name, owner]).map_err(Self::map_sql_err)?;
        return if changed != 1 {
            Err(BuckyError::from(ERROR_NOT_FOUND))
        } else {
            Ok(())
        }
    }

    async fn update_name_state(&self, name: &str, state: NameState) -> BuckyResult<()> {
        let sql = "UPDATE all_names SET name_state=?1 WHERE name_id=?2";
        let conn = self.get_conn();
        let changed = conn.execute(&sql, rusqlite::params![state as i32,name]).map_err(Self::map_sql_err)?;
        return if changed != 1 {
            Err(BuckyError::from(ERROR_NOT_FOUND))
        } else {
            Ok(())
        }
    }

    async fn update_name_rent_arrears(&self, name:&str, rent_arrears: i64) -> BuckyResult<()> {
        let sql = "UPDATE all_names SET rent_arrears=?1 WHERE name_id=?2";
        let conn = self.get_conn();
        let changed = conn.execute(&sql, rusqlite::params![rent_arrears,name]).map_err(Self::map_sql_err)?;
        return if changed != 1 {
            Err(BuckyError::from(ERROR_NOT_FOUND))
        } else {
            Ok(())
        }
    }

    async fn create_obj_desc(&self, objid:&ObjectId, desc:&SavedMetaObject) -> BuckyResult<()> {
        let sql = "SELECT update_time FROM all_descs WHERE obj_id=?1";
        let mut is_exists = false;
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &sql,
            rusqlite::params![objid.to_string()],
            |_| {
                is_exists = true;
                return Ok(());
            });

        return if let Err(err) = query_result {
            if let rusqlite::Error::QueryReturnedNoRows = err {
                //TODO:没有正确的插入desc的更新时间
                let desc_data = desc.to_vec();
                if desc_data.is_err() {
                    log::error!("serialize desc error!");
                    return Err(BuckyError::from(ERROR_PARAM_ERROR));
                }
                let desc_data_raw = desc_data.unwrap();
                let insert_sql = "INSERT INTO all_descs (obj_id,desc,update_time) VALUES (?1,?2,date('now'))";
                let insert_result = conn.execute(&insert_sql, rusqlite::params![objid.to_string(),desc_data_raw]);
                if insert_result.is_err() {
                    return Err(BuckyError::from(ERROR_EXCEPTION));
                }
                Ok(())
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        } else {
            Err(BuckyError::from(ERROR_ALREADY_EXIST))
        }
    }

    async fn get_obj_desc(&self, objid: &ObjectId) -> BuckyResult<SavedMetaObject> {
        let sql = "SELECT desc FROM all_descs WHERE obj_id=?1";
        let conn = self.get_conn();
        let query_result = conn.query_row(
            &sql,
            rusqlite::params![objid.to_string()],
            |row| {
                let desc_data: Vec<u8> = row.get(0)?;
                let desc_result = SavedMetaObject::clone_from_slice(desc_data.as_slice()).map_err(|_| rusqlite::Error::QueryReturnedNoRows)?;
                return Ok(desc_result);
            });

        return if let Err(err) = query_result {
            if let rusqlite::Error::QueryReturnedNoRows = err {
                Err(BuckyError::from(ERROR_NOT_FOUND))
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        } else {
            Ok(query_result.unwrap())
        }
    }

    async fn update_obj_desc(&self, objid:&ObjectId, desc: &SavedMetaObject, _flags:u8) -> BuckyResult<()> {
        let sql = "UPDATE all_descs SET desc=?1 WHERE obj_id=?2";
        let desc_data = desc.to_vec();
        if desc_data.is_err() {
            log::error!("serialize desc error!");
            return Err(BuckyError::from(ERROR_PARAM_ERROR));
        }
        let desc_data_raw = desc_data.unwrap();
        let conn = self.get_conn();
        let insert_result = conn.execute(&sql,rusqlite::params![desc_data_raw,objid.to_string()]);
        if insert_result.is_err() {
            return Err(BuckyError::from(ERROR_NOT_FOUND));
        }

        return Ok(());
    }

    async fn drop_desc(&self, obj_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from all_descs where obj_id=?1";
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![obj_id.to_string()])
            .map_err(Self::map_sql_err)?;
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
            let params = rusqlite::params![hash_key, offset, start_height, key, content];
            let conn = self.get_conn();
            conn.execute(sql.as_str(), params).map_err(Self::map_sql_err)?;
        } else {
            let params = rusqlite::params![key, offset, start_height, key, event.get_content()?];
            let conn = self.get_conn();
            conn.execute(sql.as_str(), params).map_err(Self::map_sql_err)?;
        }
        Ok(())
    }

    async fn get_cycle_events(&self, offset: i64, cycle: i64) -> BuckyResult<Vec<(String, i64, Event)>> {
        let table_name = self.get_cycle_event_table_name(cycle);

        let sql = format!("select param, start_height, real_key from {} where height=?1", table_name);
        let conn = self.get_conn();
        let mut stmt = conn.prepare(sql.as_str()).map_err(|e| {
            error!("prepare get cycle event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;

        let params = rusqlite::params![offset];

        let rows = stmt.query_map(params, |row|{
            let event_data:Vec<u8> = row.get(0)?;
            let start_height: i64 = row.get(1)?;
            let real_key: String = row.get(2)?;
            let event: Event = Event::clone_from_slice(event_data.as_slice()).or_else(|_| Err(rusqlite::Error::SqliteSingleThreadedMode))?;
            Ok((real_key, start_height, event))
        }).or_else(|e| {
            error!("query get cycle event err {}", e);
            if QueryReturnedNoRows == e {
                Err(ERROR_NOT_FOUND)
            } else {
                Err(ERROR_EXCEPTION)
            }
        })?;
        let mut event_list = vec![];
        for row in rows {
            event_list.push(row.or_else(|_| Err(ERROR_EXCEPTION))?);
        }
        Ok(event_list)
    }

    async fn get_all_cycle_events(&self, cycle: i64) -> BuckyResult<Vec<(String, i64, Event)>> {
        let table_name = self.get_cycle_event_table_name(cycle);

        let sql = format!("select param, start_height, real_key from {}", table_name);
        let conn = self.get_conn();
        let mut stmt = conn.prepare(sql.as_str()).map_err(|e| {
            error!("prepare get cycle event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;

        let rows = stmt.query_map(NO_PARAMS, |row|{
            let event_data:Vec<u8> = row.get(0)?;
            let start_height: i64 = row.get(1)?;
            let real_key: String = row.get(2)?;
            let event: Event = Event::clone_from_slice(event_data.as_slice()).or_else(|_| Err(rusqlite::Error::SqliteSingleThreadedMode))?;
            Ok((real_key, start_height, event))
        }).or_else(|e| {
            error!("query get cycle event err {}", e);
            if QueryReturnedNoRows == e {
                Err(BuckyError::from(ERROR_NOT_FOUND))
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        })?;
        let mut event_list = vec![];
        for row in rows {
            event_list.push(row.or_else(|_| Err(BuckyError::from(ERROR_EXCEPTION)))?);
        }
        Ok(event_list)
    }

    async fn get_cycle_event_by_key(&self, key: &str, cycle: i64) -> BuckyResult<Event> {
        let table_name = self.get_cycle_event_table_name(cycle);

        let sql = format!("select param from {} where key=?1", table_name);
        let conn = self.get_conn();
        let mut stmt = conn.prepare(sql.as_str()).map_err(|e| {
            error!("prepare get cycle event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;

        if key.len() > 64 {
            let mut hasher = Sha256::new();
            hasher.input(key);
            let hash_key = hex::encode(hasher.result());
            let params = rusqlite::params![hash_key];

            stmt.query_row(params, |row|{
                let event_data:Vec<u8> = row.get(0)?;
                let event: Event = Event::clone_from_slice(event_data.as_slice()).or_else(|_| Err(rusqlite::Error::SqliteSingleThreadedMode))?;
                Ok(event)
            }).or_else(|e| {
                error!("query get cycle event err {}, key {}", e, key);
                if QueryReturnedNoRows == e {
                    Err(BuckyError::from(ERROR_NOT_FOUND))
                } else {
                    Err(BuckyError::from(ERROR_EXCEPTION))
                }
            })
        } else {
            let params = rusqlite::params![key];
            stmt.query_row(params, |row|{
                let event_data:Vec<u8> = row.get(0)?;
                let event: Event = Event::clone_from_slice(event_data.as_slice()).or_else(|_| Err(rusqlite::Error::SqliteSingleThreadedMode))?;
                Ok(event)
            }).or_else(|e| {
                error!("query get cycle event err {}, key {}", e, key);
                if QueryReturnedNoRows == e {
                    Err(BuckyError::from(ERROR_NOT_FOUND))
                } else {
                    Err(BuckyError::from(ERROR_EXCEPTION))
                }
            })
        }
    }

    async fn drop_cycle_event(&self, key: &str, cycle: i64) -> BuckyResult<()> {
        let table_name = self.get_cycle_event_table_name(cycle);
        let sql = format!("delete from {} where key=?1", table_name);

        if key.len() > 64 {
            let mut hasher = Sha256::new();
            hasher.input(key);
            let hash_key = hex::encode(hasher.result());
            let params = rusqlite::params![hash_key];
            let conn = self.get_conn();
            conn.execute(sql.as_str(), params).map_err(|e|{
                error!("remove events at key {} err {}", key, e);
                BuckyError::from(ERROR_EXCEPTION)
            })?;
        } else {
            let params = rusqlite::params![key];
            let conn = self.get_conn();
            conn.execute(sql.as_str(), params).map_err(|e|{
                error!("remove events at key {} err {}", key, e);
                BuckyError::from(ERROR_EXCEPTION)
            })?;
        }

        Ok(())
    }

    async fn drop_all_cycle_events(&self, cycle: i64) -> BuckyResult<()> {
        let table_name = self.get_cycle_event_table_name(cycle);
        let sql = format!("delete from {}", table_name);

        let conn = self.get_conn();
        conn.execute(sql.as_str(), NO_PARAMS).map_err(|e|{
            error!("remove events err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;

        Ok(())
    }

    async fn add_or_update_event(&self, key: &str, event: Event, height: i64) -> BuckyResult<()> {
        let sql = "insert into event VALUES(?1, ?2, ?3, ?4) ON CONFLICT(type, key,height) DO UPDATE SET param=?3,height=?4";
        let event_raw = event.get_content()?;
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![event.get_type() as u8, key, event_raw, height]).map_err(|e| {
            error!("execute add event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        Ok(())
    }

    async fn get_event(&self, event_type: EventType, height: i64) -> BuckyResult<Vec<Event>> {
        let sql = "select param from event where type=?1 and height=?2";
        let conn = self.get_conn();
        let mut stmt = conn.prepare(sql).map_err(|e| {
            error!("prepare get event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        let rows = stmt.query_map(rusqlite::params![event_type as i32, height], |row|{
            let event_data:Vec<u8> = row.get(0)?;
            Ok(event_data)
        }).map_err(|e|{
            error!("query get event err {}, type {}, height {}", e, event_type as i32, height);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        let mut ret = vec![];
        for row in rows {
            if let Ok(event_data) = row {
                match Event::clone_from_slice(event_data.as_slice()) {
                    Ok(event) => {
                        ret.push(event);
                    },
                    Err(e) => {
                        error!("parse event fail {}", e);
                    },
                }
            }
        }
        Ok(ret)
    }

    async fn get_event_by_key(&self, key: &str, event_type: EventType) -> BuckyResult<Vec<(Event, i64)>> {
        let sql = "select param,height from event where type=?1 and key=?2";
        let conn = self.get_conn();
        let mut stmt = conn.prepare(sql).map_err(|e| {
            error!("prepare get event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        let rows = stmt.query_map(rusqlite::params![event_type as i32, key], |row|{
            let event_data:Vec<u8> = row.get(0)?;
            let height:i64 = row.get(1)?;
            Ok((event_data, height))
        }).map_err(|e|{
            error!("query get event err {}, type {}, key {}", e, event_type as i32, key);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        let mut ret = vec![];
        for row in rows {
            if let Ok((event_data, height)) = row {
                match Event::clone_from_slice(event_data.as_slice()) {
                    Ok(event) => {
                        ret.push((event, height));
                    },
                    Err(e) => {
                        error!("parse event fail {}", e);
                    },
                }
            }
        }
        Ok(ret)
    }

    async fn drop_event(&self, height: i64) -> BuckyResult<()> {
        let sql = "delete from event where height<=?1";
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![height]).map_err(|e|{
            error!("remove events at height {} err {}", height, e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        Ok(())
    }

    async fn add_or_update_once_event(&self, key: &str, event: &Event, height: i64) -> BuckyResult<()> {
        let sql = "insert into once_event (key, height, param) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET param=?3,height=?2";
        let event_raw = event.get_content()?;
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![key, height, event_raw]).map_err(|e| {
            error!("execute add event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        Ok(())
    }

    async fn get_once_events(&self, height: i64) -> BuckyResult<Vec<Event>> {
        let sql = "select param from once_event where height=?1";
        let conn = self.get_conn();
        let mut stmt = conn.prepare(sql).map_err(|e| {
            error!("prepare get event err {}", e);
            ERROR_EXCEPTION
        })?;
        let rows = stmt.query_map(rusqlite::params![height], |row|{
            let event_data:Vec<u8> = row.get(0)?;
            Ok(event_data)
        }).map_err(|e|{
            error!("query get event err {}, height {}", e, height);
            ERROR_EXCEPTION
        })?;
        let mut ret = vec![];
        for row in rows {
            if let Ok(event_data) = row {
                match Event::clone_from_slice(event_data.as_slice()) {
                    Ok(event) => {
                        ret.push(event);
                    },
                    Err(e) => {
                        error!("parse event fail {}", e);
                    },
                }
            }
        }
        Ok(ret)
    }

    async fn get_once_event_by_key(&self, key: &str) -> BuckyResult<Event> {
        let sql = "select param from once_event where key=?1";
        let conn = self.get_conn();
        let mut stmt = conn.prepare(sql).map_err(|e| {
            error!("prepare get event err {}", e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        stmt.query_row(rusqlite::params![key], |row|{
            let event_data:Vec<u8> = row.get(0)?;
            let event: Event = Event::clone_from_slice(event_data.as_slice()).or_else(|_| Err(rusqlite::Error::SqliteSingleThreadedMode))?;
            Ok(event)
        }).or_else(|e| {
            error!("query get event err {}, key {}", e, key);
            if QueryReturnedNoRows == e {
                Err(BuckyError::from(ERROR_NOT_FOUND))
            } else {
                Err(BuckyError::from(ERROR_EXCEPTION))
            }
        })
    }

    async fn drop_once_event(&self, height: i64) -> BuckyResult<()> {
        let sql = "delete from once_event where height<=?1";
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![height]).map_err(|e|{
            error!("remove events at height {} err {}", height, e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        Ok(())
    }

    async fn drop_once_event_by_key(&self, key: &str) -> BuckyResult<()> {
        let sql = "delete from once_event where key=?1";
        let conn = self.get_conn();
        conn.execute(sql, rusqlite::params![key]).map_err(|e|{
            error!("remove event at key {} err {}", key, e);
            BuckyError::from(ERROR_EXCEPTION)
        })?;
        Ok(())
    }
}

pub struct SqliteStorage {
    path: PathBuf
}

#[async_trait]
impl Storage for SqliteStorage {
    fn path(&self) -> &Path {
        self.path.as_path()
    }

    async fn create_state(&self) -> StateRef {
        if *self.path.as_path() == *storage_in_mem_path() {
            SqliteState::new(rusqlite::Connection::open_in_memory().unwrap())
        } else {
            SqliteState::new(rusqlite::Connection::open(self.path.as_path()).unwrap())
        }
    }

    fn state_hash(&self) -> BuckyResult<StateHash> {
        static SQLITE_HEADER_SIZE: usize = 100;
        let content = std::fs::read(self.path()).map_err(|err| {
            error!("read file {} fail, err {}", self.path.display(), err);
            ERROR_NOT_FOUND})?;
        let mut hasher = Sha256::new();
        hasher.input(&content[SQLITE_HEADER_SIZE..]);
        Ok(HashValue::from(hasher.result()))
    }
}

pub fn new_sqlite_storage(path: &Path) -> StorageRef {
    Box::new(SqliteStorage {
        path: PathBuf::from(path.to_str().unwrap())
    })
}

#[cfg(test)]
pub mod sqlite_storage_tests {
    use crate::{new_sqlite_storage, SqliteStorage, SqliteState, SqlState, MetaConnection, new_sql_storage};
    use crate::state_storage::{Storage, storage_in_mem_path, StateRef, StorageRef};
    use cyfs_base_meta::{GenesisConfig, GenesisPriceConfig, GenesisCoinConfig, StateHash};
    use cyfs_base::{ObjectId, BuckyResult, NameInfo, NameRecord, NameLink, HashValue};
    use std::path::Path;
    use std::collections::HashMap;
    use async_trait::async_trait;
    use sqlx::Connection;

    pub struct TestStorage {
        storage: SqliteStorage,
        state: StateRef,
    }

    unsafe impl Send for TestStorage {

    }

    #[async_trait]
    impl Storage for TestStorage {
        fn path(&self) -> &Path {
            self.storage.path.as_path()
        }

        async fn create_state(&self) -> StateRef {
            self.state.clone()
        }

        fn state_hash(&self) -> BuckyResult<StateHash> {
            Ok(HashValue::default())
        }
    }

    pub async fn create_test_storage() -> StorageRef {
        let state = SqlState::new(MetaConnection::connect("sqlite::memory:").await.unwrap());
        state.init_genesis(&GenesisConfig {
            chain_type: Some("".to_string()),
            coinbase: ObjectId::default(),
            interval: 10,
            bfc_spv_node: "http://127.0.0.1:11998".to_string(),
            coins: vec![GenesisCoinConfig {
                coin_id: 0,
                pre_balance: Vec::new()
            }],
            price: GenesisPriceConfig{}
        }).await.unwrap();
        state.init().await.unwrap();
        Box::new(TestStorage {
            storage: SqliteStorage {
                path: storage_in_mem_path().to_path_buf()
            },
            state
        })
    }

    pub async fn create_state() -> StateRef {
        let state = new_sql_storage(storage_in_mem_path()).create_state().await;
        state.init_genesis(&GenesisConfig {
            chain_type: Some("".to_string()),
            coinbase: ObjectId::default(),
            interval: 10,
            bfc_spv_node: "http://127.0.0.1:11998".to_string(),
            coins: vec![GenesisCoinConfig {
                coin_id: 0,
                pre_balance: Vec::new()
            }],
            price: GenesisPriceConfig{}
        }).await.unwrap();
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
