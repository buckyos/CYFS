use crate::*;
use crate::archive_storage::{Archive, ArchiveStorage, ArchiveStorageRef};
use async_trait::async_trait;
use cyfs_base::*;
use sqlx::Row;
use crate::helper::get_meta_err_code;
use async_std::sync::{Mutex, MutexGuard, Arc};
use std::collections::HashMap;
use std::sync::RwLock;
use std::path::{PathBuf, Path};
use std::time::Duration;
use async_std::prelude::StreamExt;

use super::db_helper::*;

#[derive(Clone, Debug)]
pub struct Stat {
    id: String,
    key: u8,
    value: u64,
    extra: u64,
}
pub struct SqlArchive {
    pool: SqlPool,
    trace: bool,
    stat: RwLock<HashMap<String, Stat>>,
}

pub type ArchiveRef = std::sync::Arc<SqlArchive>;
pub type ArchiveWeakRef = std::sync::Weak<SqlArchive>;

impl SqlArchive {
    pub async fn new(path: &str, trace: bool) -> ArchiveRef {
        let pool = SqlPool::open(
            format!(
                "sqlite://{}",
                path
            )
            .as_str(),
            10,
        )
        .await.unwrap();

        let ret = ArchiveRef::new(SqlArchive {
            pool,
            trace,
            stat: RwLock::new(HashMap::new()),
        });

        let manager = ret.clone();
        async_std::task::spawn(async move {
            // 默认每1分钟存一次
            let mut interval = async_std::stream::interval(Duration::from_secs(10));
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
        }

        if empty.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.get_conn().await?;
        conn.begin_transaction().await?;
        log::info!("size: {}", empty.len());
        for (key, stat) in empty.iter() {
            log::info!("key: {:?}, stat: {:?}", key, stat);
            if key.contains("_desc") {
                // device_stat
                let sql = format!("SELECT update_time FROM device_stat WHERE obj_id='{}'", stat.id.to_owned());
                let query_result = conn.query_one(sql_query(sql.as_str())).await;
                if let Err(err) = query_result {
                    log::error!("err: {:?}, code: {}", err, get_meta_err_code(&err)?);
                    if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                        let insert_sql = format!("INSERT INTO device_stat(obj_id,obj_type,height,create_time,update_time) VALUES ('{}',{},{},{},{})",stat.id.to_owned(),stat.key,stat.extra,stat.value,stat.value);
                        conn.execute_sql(sql_query(insert_sql.as_str())).await?;
                        log::info!("1");
                    }
                } else {
                    let sql = format!("UPDATE device_stat SET update_time={} WHERE obj_id='{}'", stat.value, stat.id.to_owned());
                    conn.execute_sql(sql_query(sql.as_str())).await?;
                    log::info!("2");
                }
            }
            if key.contains("_meta_obj") {
                // meta_object_stat
                let sql = format!("SELECT success, failed FROM meta_object_stat WHERE id='{}'", stat.id.to_owned());
                let query_result = conn.query_one(sql_query(sql.as_str())).await;
                if let Err(err) = query_result {
                    log::info!("333333");
                    if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                        let now = bucky_time_now();
                        let insert_sql = format!("INSERT INTO meta_object_stat(id,success,failed,create_time) VALUES ('{}',{},{},{})", stat.id.to_owned(), stat.value, stat.extra, now);
                        conn.execute_sql(sql_query(insert_sql.as_str())).await?;
                        log::info!("3");
                    }
                } else {
                    log::info!("44444");
                    let row = query_result?;
                    let temp_success: i64 = row.get("success");
                    let temp_failed: i64 = row.get("failed");

                    let success = temp_success + stat.value as i64;
                    let failed  = temp_failed + stat.extra as i64;
                    let sql = format!("UPDATE meta_object_stat SET success={}, failed={} WHERE id='{}'", success, failed, stat.id.to_owned());
                    conn.execute_sql(sql_query(sql.as_str())).await?;
                    log::info!("4");
                }
            }
            
            if key.contains("_meta_api") {
                // meta_api_stat
                let sql = format!("SELECT success, failed FROM meta_api_stat WHERE id='{}'", stat.id.to_owned());
                let query_result = conn.query_one(sqlx::query(sql.as_str())).await?;
                let insert_sql = format!("INSERT INTO meta_api_stat(id,success,failed) VALUES ('{}',{},{})", stat.id.to_owned(), stat.value, stat.extra);
                conn.execute_sql(sqlx::query(insert_sql.as_str())).await?;
                log::info!("5");
                if query_result.is_empty() {
                    let insert_sql = format!("INSERT INTO meta_api_stat(id,success,failed) VALUES ('{}',{},{})", stat.id.to_owned(), stat.value, stat.extra);
                    conn.execute_sql(sqlx::query(insert_sql.as_str())).await?;
                } else {
                    let row = query_result;
                    let temp_success: i64 = row.get("success");
                    let temp_failed: i64 = row.get("failed");

                    let success = temp_success + stat.value as i64;
                    let failed  = temp_failed + stat.extra as i64;
                    let sql = format!("UPDATE meta_api_stat SET success={}, failed={} WHERE id='{}'", success, failed, stat.id.to_owned());
                    conn.execute_sql(sqlx::query(sql.as_str())).await?;
                    log::info!("7");
                }
            }
        }

        conn.commit_transaction().await?;
        log::info!("11111");
        Ok(())

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
        
        let mut conn = self.pool.get_conn().await?;
        conn.execute_sql(sql_query(sql.as_str())).await?;
        Ok(())
    }

    async fn init_api_stat_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "meta_api_stat" (
            "id"	    CHAR(64) PRIMARY KEY NOT NULL UNIQUE,
            "success"	INTEGER NOT NULL,
            "failed"	INTEGER NOT NULL
        )"#;
        let mut conn = self.pool.get_conn().await?;
        conn.execute_sql(sql_query(sql)).await?;
        Ok(())
    }

    async fn init_meta_object_tbl(&self) -> BuckyResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS "meta_object_stat" (
            "id"	        CHAR(64) PRIMARY KEY NOT NULL UNIQUE,
            "success"	    INTEGER NOT NULL,
            "failed"	    INTEGER NOT NULL,
            "create_time"	INTEGER NOT NULL
        )"#;
        let mut conn = self.pool.get_conn().await?;
        conn.execute_sql(sql_query(sql)).await?;
        Ok(())
    }
}

#[async_trait]
impl Archive for SqlArchive {

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
    archive: ArchiveRef,
}
#[async_trait]
impl ArchiveStorage for SqlArchiveStorage {
    fn path(&self) -> &Path {
        self.path.as_path()
    }
    async fn create_archive(&self, _read_only: bool) -> &ArchiveRef {
        let _locker = self.get_locker().await;
        return &self.archive;
    }

    async fn get_locker(&self) -> MutexGuard<'_, ()> {
        self.locker.lock().await
    }
}

pub fn new_archive_storage(path: &Path, trace: bool) -> ArchiveStorageRef {
    let archive = async_std::task::block_on(async {SqlArchive::new(path.to_str().unwrap(), trace).await});
    let _ = async_std::task::block_on(async {archive.init().await});
    Arc::new(Box::new(SqlArchiveStorage {
        path: PathBuf::from(path.to_str().unwrap()),
        trace,
        locker: Default::default(),
        archive,
    }))
}