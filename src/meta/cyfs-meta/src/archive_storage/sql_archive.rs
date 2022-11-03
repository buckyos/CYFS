use crate::*;
use crate::archive_storage::{Archive, ArchiveStorage, storage_in_mem_path, ArchiveStorageRef};
use async_trait::async_trait;
use cyfs_base::*;
use sqlx::{Row, Connection, ConnectOptions};
use crate::helper::get_meta_err_code;
use async_std::sync::{Mutex, MutexGuard, Arc};
use std::str::FromStr;
use log::*;
use sha2::{Sha256, Digest};
use std::path::{PathBuf, Path};
use std::time::Duration;
use sqlx::sqlite::{SqliteJournalMode};
pub struct SqlArchive {
    conn: Mutex<ArchiveConnection>,
    transaction_seq: Mutex<i32>,
}

pub type ArchiveRef = std::sync::Arc<SqlArchive>;
pub type ArchiveWeakRef = std::sync::Weak<SqlArchive>;

impl SqlArchive {
    pub fn new(conn: ArchiveConnection) -> ArchiveRef {
        ArchiveRef::new(SqlArchive {
            conn: Mutex::new(conn),
            transaction_seq: Mutex::new(0),
        })
    }

    pub async fn get_conn(&self) -> MutexGuard<'_, ArchiveConnection> {
        self.conn.lock().await
    }

    fn stat_tbl_name(&self) -> &'static str {
        static STAT_TBL_NAME: &str = "device_stat";
        STAT_TBL_NAME
    }

    async fn init_obj_stat_table(&self) -> BuckyResult<()> {
        let sql = format!("CREATE TABLE IF NOT EXISTS \"{}\"
            (\"obj_id\" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
            \"obj_type\" INTEGER NOT NULL,
            \"create_time\" INTEGER NOT NULL,
            \"update_time\" INTEGER NOT NULL);", self.stat_tbl_name());
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql.as_str())).await?;

        Ok(())
    }

    async fn init_api_stat_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "meta_api_stat" (
            "id"	    CHAR(64) PRIMARY KEY NOT NULL UNIQUE,
            "success"	INTEGER NOT NULL,
            "failed"	INTEGER NOT NULL
        )"#;

        self.get_conn().await.execute_sql(sqlx::query(sql)).await?;
        Ok(())
    }

    async fn init_meta_object_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "meta_object_stat" (
            "id"	        CHAR(64) PRIMARY KEY NOT NULL UNIQUE,
            "success"	    INTEGER NOT NULL,
            "failed"	    INTEGER NOT NULL,
            "create_time"	INTEGER NOT NULL
        )"#;

        self.get_conn().await.execute_sql(sqlx::query(sql)).await?;
        Ok(())
    }
}

#[async_trait]
impl Archive for SqlArchive {
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
        self.init_api_stat_tbl().await?;
        self.init_meta_object_tbl().await?;
        self.init_obj_stat_table().await?;

        Ok(())
    }

    async fn create_or_update_desc_stat(&self, objid: &ObjectId, obj_type: u8) -> BuckyResult<()> {
        let sql = "SELECT update_time FROM device_stat WHERE obj_id=?1";
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(sql).bind(objid.to_string())).await;
        return if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                let insert_sql = "INSERT INTO device_stat(obj_id,obj_type,create_time,update_time) VALUES (?1,?2,?3,?4)";
                let now = bucky_time_now();
                conn.execute_sql(sqlx::query(insert_sql).bind(objid.to_string()).bind(obj_type).bind(now as i64).bind(now as i64)).await?;
                Ok(())
            } else {
                Err(crate::meta_err!(ERROR_EXCEPTION))
            }
        } else {
            let sql = "UPDATE device_stat SET update_time=?1 WHERE obj_id=?2";
            let now = bucky_time_now();
            conn.execute_sql(sqlx::query(sql).bind(now as i64).bind(objid.to_string())).await?;
    
            Ok(())
        }
    }

    // people/device 数目
    async fn get_obj_desc_stat(&self, obj_type: u8) -> BuckyResult<u64> {
        let sql = "SELECT count(*) FROM device_stat WHERE obj_type=?1";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(obj_type)).await?;

        let sum: i64 = row.try_get(0).unwrap_or(0);
        Ok(sum as u64)
    }

    // people/device 每日新增
    async fn get_daily_added_desc(&self, obj_type: u8, date: u64) -> BuckyResult<u64> {
        let sql = "SELECT count(*) FROM device_stat WHERE obj_type=?1 and create_time=?2";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(obj_type).bind(date as i64)).await?;

        let sum: i64 = row.try_get(0).unwrap_or(0);
        Ok(sum as u64)
    }

    // people/device 每日活跃
    async fn get_daily_active_desc(&self, obj_type: u8, date: u64) -> BuckyResult<u64> {
        let sql = "SELECT count(*) FROM device_stat WHERE obj_type=?1 and update_time=?2";
        let mut conn = self.get_conn().await;
        let row = conn.query_one(sqlx::query(sql).bind(obj_type).bind(date as i64)).await?;

        let sum: i64 = row.try_get(0).unwrap_or(0);
        Ok(sum as u64)
    }

    async fn drop_desc_stat(&self, obj_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from device_stat where obj_id=?1";
        let mut conn = self.get_conn().await;
        conn.execute_sql(sqlx::query(sql).bind(obj_id.to_string())).await?;
        Ok(())
    }

    async fn set_meta_object_stat(&self, objid: &ObjectId, status: u8) -> BuckyResult<()> {
        let mut success: i64 = 0;
        let mut failed:i64 = 0;
        if status == 0 {
            success = 1;
        } else {
            failed = 1;
        }
        let sql = "SELECT success, failed FROM meta_object_stat WHERE id=?1";
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(sql).bind(objid.to_string())).await;
        return if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                let now = bucky_time_now();
                let insert_sql = "INSERT INTO meta_object_stat(id,success,failed,create_time) VALUES (?1,?2,?3,?4)";
                conn.execute_sql(sqlx::query(insert_sql).bind(objid.to_string()).bind(success).bind(failed).bind(now as i64)).await?;
                Ok(())
            } else {
                Err(crate::meta_err!(ERROR_EXCEPTION))
            }
        } else {
            let row = query_result?;
            let temp_success: i64 = row.get("success");
            let temp_failed: i64 = row.get("failed");

            success += temp_success;
            failed  += temp_failed;
            let sql = "UPDATE meta_object_stat SET success=?1, failed=?2 WHERE id=?3";
            conn.execute_sql(sqlx::query(sql).bind(success).bind(failed).bind(objid.to_string())).await?;
            Ok(())
        }
    }


    async fn set_meta_api_stat(&self, id: &str, status: u8) -> BuckyResult<()> {
        let mut success: i64 = 0;
        let mut failed:i64 = 0;
        if status == 0 {
            success = 1;
        } else {
            failed = 1;
        }
        let sql = "SELECT success, failed FROM meta_api_stat WHERE id=?1";
        let mut conn = self.get_conn().await;
        let query_result = conn.query_one(sqlx::query(sql).bind(id)).await;
        return if let Err(err) = query_result {
            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                let insert_sql = "INSERT INTO meta_api_stat(id,success,failed) VALUES (?1,?2,?3)";
                conn.execute_sql(sqlx::query(insert_sql).bind(id).bind(success).bind(failed)).await?;
                Ok(())
            } else {
                Err(crate::meta_err!(ERROR_EXCEPTION))
            }
        } else {
            let row = query_result?;
            let temp_success: i64 = row.get("success");
            let temp_failed: i64 = row.get("failed");

            success += temp_success;
            failed  += temp_failed;
            let sql = "UPDATE meta_api_stat SET success=?1, failed=?2 WHERE id=?3";
            conn.execute_sql(sqlx::query(sql).bind(success).bind(failed).bind(id)).await?;
            Ok(())
        }
    }
}


pub struct SqlArchiveStorage {
    path: PathBuf,
    locker: Mutex<()>,
}
#[async_trait]
impl ArchiveStorage for SqlArchiveStorage {
    fn path(&self) -> &Path {
        self.path.as_path()
    }

    async fn create_archive(&self, _read_only: bool) -> ArchiveRef {
        let _locker = self.get_locker().await;
        if *self.path.as_path() == *storage_in_mem_path() {
            let mut options = MetaConnectionOptions::new()
                .journal_mode(SqliteJournalMode::Memory);
            options.log_statements(LevelFilter::Off)
                .log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
            SqlArchive::new(options.connect().await.unwrap())
        } else {
            let path = self.path.to_str().unwrap();
            // info!("open db:{}", path);
            let mut options = MetaConnectionOptions::from_str(format!("sqlite://{}", path).as_str()).unwrap()
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Memory);
            options.log_statements(LevelFilter::Off)
                .log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
            let conn = MetaConnection::connect_with(&options).await;
            if let Err(e) = &conn {
                let msg = format!("{:?}", e);
                info!("{}", msg);
            }
            let conn = conn.unwrap();
            SqlArchive::new(conn)
        }
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

pub fn new_archive_storage(path: &Path) -> ArchiveStorageRef {
    Arc::new(Box::new(SqlArchiveStorage {
        path: PathBuf::from(path.to_str().unwrap()),
        locker: Default::default()
    }))
}