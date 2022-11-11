use crate::*;
use crate::archive_storage::{Archive, ArchiveStorage, storage_in_mem_path, ArchiveStorageRef};
use async_trait::async_trait;
use cyfs_base::*;
use sqlx::{Row, Connection, ConnectOptions};
use crate::helper::get_meta_err_code;
use async_std::sync::{Mutex, MutexGuard, Arc};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::RwLock;
use log::*;
use std::path::{PathBuf, Path};
use std::time::Duration;
use sqlx::sqlite::{SqliteJournalMode};
use async_std::prelude::StreamExt;

#[derive(Clone, Debug)]
pub struct Stat {
    id: String,
    key: u8,
    value: u64,
    extra: u64,
}
pub struct SqlArchive {
    conn: Mutex<ArchiveConnection>,
    transaction_seq: Mutex<i32>,
    trace: bool,
    stat: RwLock<HashMap<String, Stat>>,
}

pub type ArchiveRef = std::sync::Arc<SqlArchive>;
pub type ArchiveWeakRef = std::sync::Weak<SqlArchive>;

impl SqlArchive {
    pub fn new(conn: ArchiveConnection, trace: bool) -> ArchiveRef {
        let ret = ArchiveRef::new(SqlArchive {
            conn: Mutex::new(conn),
            transaction_seq: Mutex::new(0),
            trace,
            stat: RwLock::new(HashMap::new()),
        });

        let manager = ret.clone();
        async_std::task::spawn(async move {
            // 默认每1分钟存一次
            let mut interval = async_std::stream::interval(Duration::from_secs(60));
            while let Some(_) = interval.next().await {
                let _ = manager.inner_save().await;
            }
        });

        return ret;
    }

    async fn inner_save(&self) -> BuckyResult<()> {
        if !self.trace {
            return Ok(());
        }

        let mut empty = HashMap::new();
        {
            let mut data = self.stat.write().unwrap();
            std::mem::swap(&mut *data, &mut empty);
            //debug!("stat: {}, empty: {}", data.len(), empty.len());
        }

        if empty.is_empty() {
            return Ok(());
        }

        let mut conn = self.get_conn().await;
        
        for (key, stat) in empty.iter() {

            if key.contains("_desc") {
                // device_stat
                let sql = "SELECT update_time FROM device_stat WHERE obj_id=?1";

                let query_result = conn.query_one(sqlx::query(sql).bind(stat.id.to_owned())).await;
                if let Err(err) = query_result {
                    if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                        let insert_sql = "INSERT INTO device_stat(obj_id,obj_type,height,create_time,update_time) VALUES (?1,?2,?3,?4,?5)";
                        conn.execute_sql(sqlx::query(insert_sql).bind(stat.id.to_owned()).bind(stat.key).bind(stat.extra as i64).bind(stat.value as i64).bind(stat.value as i64)).await?;
                    } else {
                    }
                } else {
                    let sql = "UPDATE device_stat SET update_time=?1 WHERE obj_id=?2";
                    conn.execute_sql(sqlx::query(sql).bind(stat.value as i64).bind(stat.id.to_owned())).await?;
                }
            }
            if key.contains("_meta_obj") {
                // meta_object_stat
                let sql = "SELECT success, failed FROM meta_object_stat WHERE id=?1";
                let query_result = conn.query_one(sqlx::query(sql).bind(stat.id.to_owned())).await;
                if let Err(err) = query_result {
                    if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                        let now = bucky_time_now();
                        let insert_sql = "INSERT INTO meta_object_stat(id,success,failed,create_time) VALUES (?1,?2,?3,?4)";
                        conn.execute_sql(sqlx::query(insert_sql).bind(stat.id.to_owned()).bind(stat.value as i64).bind(stat.extra as i64).bind(now as i64)).await?;
                    }
                } else {
                    let row = query_result?;
                    let temp_success: i64 = row.get("success");
                    let temp_failed: i64 = row.get("failed");

                    let success = temp_success + stat.value as i64;
                    let failed  = temp_failed + stat.extra as i64;
                    let sql = "UPDATE meta_object_stat SET success=?1, failed=?2 WHERE id=?3";
                    conn.execute_sql(sqlx::query(sql).bind(success).bind(failed).bind(stat.id.to_owned())).await?;
                }

            }
            
            if key.contains("_meta_api") {
                // meta_api_stat
                let sql = "SELECT success, failed FROM meta_api_stat WHERE id=?1";
                let query_result = conn.query_one(sqlx::query(sql).bind(stat.id.to_owned())).await;
                if let Err(err) = query_result {
                    if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                        let insert_sql = "INSERT INTO meta_api_stat(id,success,failed) VALUES (?1,?2,?3)";
                        conn.execute_sql(sqlx::query(insert_sql).bind(stat.id.to_owned()).bind(stat.value as i64).bind(stat.extra as i64)).await?;
                    }
                } else {
                    let row = query_result?;
                    let temp_success: i64 = row.get("success");
                    let temp_failed: i64 = row.get("failed");

                    let success = temp_success + stat.value as i64;
                    let failed  = temp_failed + stat.extra as i64;
                    let sql = "UPDATE meta_api_stat SET success=?1, failed=?2 WHERE id=?3";
                    conn.execute_sql(sqlx::query(sql).bind(success).bind(failed).bind(stat.id.to_owned())).await?;
                }
            }
        }

        Ok(())

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
            \"height\" INTEGER NOT NULL,
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
        if self.trace {
            self.init_api_stat_tbl().await?;
            self.init_meta_object_tbl().await?;
            self.init_obj_stat_table().await?;
        }
        Ok(())
    }

    async fn create_or_update_desc_stat(&self, objid: &ObjectId, obj_type: u8, height: u64) -> BuckyResult<()> {
        if !self.trace {
            return Ok(());
        }

        if let Ok( mut lock) = self.stat.write() {
            let id = format!("{}_desc", objid.to_string());
            let now = bucky_time_now();
            lock.insert(id, Stat { id: objid.to_string(), key: obj_type, value: now, extra: height });
        }

        Ok(())
    }

    async fn set_meta_object_stat(&self, objid: &ObjectId, status: u8) -> BuckyResult<()> {
        if !self.trace {
            return Ok(());
        }
        if let Ok( mut lock) = self.stat.write() {
            let id = format!("{}_meta_obj", objid.to_string());
            let stat = &Stat { id: objid.to_string(), key: status, value: 0, extra: 0  };
            let cur_stat = lock.get(&id).unwrap_or(stat);
            let mut success: u64 = cur_stat.value;
            let mut failed:u64 = cur_stat.extra;
            if status == 0 {
                success += 1;
            } else {
                failed += 1;
            }
            lock.insert(id, Stat { id: objid.to_string(), key: status, value: success, extra: failed  });
        }
        Ok(())
    }

    async fn set_meta_api_stat(&self, api: &str, status: u8) -> BuckyResult<()> {
        if !self.trace {
            return Ok(());
        }

        if let Ok( mut lock) = self.stat.write() {
            let id = format!("{}_meta_api", api);
            let stat = &Stat { id: api.to_string(), key: status, value: 0, extra: 0  };

            let cur_stat = lock.get(&id).unwrap_or(stat);
            let mut success: u64 = cur_stat.value;
            let mut failed:u64 = cur_stat.extra;
            if status == 0 {
                success += 1;
            } else {
                failed += 1;
            }
            lock.insert(id, Stat { id: api.to_string(), key: status, value: success, extra: failed  });
        }

        Ok(())
    }
}

pub struct SqlArchiveStorage {
    path: PathBuf,
    trace: bool,
    locker: Mutex<()>,
}
#[async_trait]
impl ArchiveStorage for SqlArchiveStorage {
    fn path(&self) -> &Path {
        self.path.as_path()
    }
    async fn create_archive(&self, _read_only: bool) -> ArchiveRef {
        let _locker = self.get_locker().await;
        if *self.path.as_path() == *storage_in_mem_path() || !self.trace {
            let mut options = MetaConnectionOptions::new()
                .journal_mode(SqliteJournalMode::Memory);
            options.log_statements(LevelFilter::Off)
                .log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
            SqlArchive::new(options.connect().await.unwrap(), self.trace)
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
            SqlArchive::new(conn, self.trace)
        }
    }

    async fn get_locker(&self) -> MutexGuard<'_, ()> {
        self.locker.lock().await
    }
}

pub fn new_archive_storage(path: &Path, trace: bool) -> ArchiveStorageRef {
    Arc::new(Box::new(SqlArchiveStorage {
        path: PathBuf::from(path.to_str().unwrap()),
        trace,
        locker: Default::default()
    }))
}