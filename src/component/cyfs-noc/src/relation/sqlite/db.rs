use super::data::*;
use super::sql::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::SqliteConnectionHolder;

use rusqlite::{params, OptionalExtension};
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct SqliteDBObjectRelationCache {
    data_file: PathBuf,
    conn: Arc<SqliteConnectionHolder>,
}

impl SqliteDBObjectRelationCache {
    pub fn new(isolate: &str) -> BuckyResult<Self> {
        let dir = cyfs_util::get_cyfs_root_path().join("data");
        let dir = if isolate.len() > 0 {
            dir.join(isolate)
        } else {
            dir
        };
        let dir = dir.join("named-object-cache");

        if !dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let msg = format!(
                    "create named object cache dir error! dir={}, err={}",
                    dir.display(),
                    e
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        let data_file = dir.join("cache.db");

        // 需要在开启connection之前调用
        let file_exists = data_file.exists();

        info!(
            "named object relation cache sqlite db file: {}, exists={}",
            data_file.display(),
            file_exists
        );

        let ret = Self {
            data_file: data_file.clone(),
            conn: Arc::new(SqliteConnectionHolder::new(data_file)),
        };

        if !file_exists {
            if let Err(e) = ret.init_db() {
                error!(
                    "init named object relation cache db error! now will delete file, {}",
                    e
                );
                if let Err(e) = std::fs::remove_file(&ret.data_file) {
                    error!("remove db file error: {}", e);
                }

                return Err(e);
            }
        } else {
            ret.check_and_update()?;
        }

        Ok(ret)
    }

    fn init_db(&self) -> BuckyResult<()> {
        let (conn, _lock) = self.conn.get_write_conn()?;

        for sql in &INIT_OBJECT_RELATION_CACHE_LIST {
            info!("will exec: {}", sql);
            conn.execute(&sql, []).map_err(|e| {
                let msg = format!(
                    "init named object relation cache table error! sql={}, err={}",
                    sql, e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }

        info!("init named object cache relation cache sqlite table success!");

        Ok(())
    }

    fn check_and_update(&self) -> BuckyResult<()> {
        Ok(())
    }

    // 判断是不是相同主键的项目已经存在
    fn is_exists_error(e: &rusqlite::Error) -> bool {
        match e {
            rusqlite::Error::SqliteFailure(e, _) => {
                if e.code == rusqlite::ErrorCode::ConstraintViolation {
                    return true;
                }
            }
            _ => {}
        }

        false
    }

    pub fn put(&self, req: &NamedObjectRelationCachePutRequest) -> BuckyResult<()> {
        let object_id = req.cache_key.object_id.to_string();
        let now = bucky_time_now() as i64;
        let relation_type: u8 = req.cache_key.relation_type.into();
        let target_object_id = req.target_object_id.to_string();

        let params = params![
            &object_id,
            relation_type,
            req.cache_key.relation,
            &target_object_id,
            &now,
            &now
        ];

        let put_sql = r#"
            INSERT OR REPLACE INTO cache_object_relation (object_id, relation_type, relation, target_object_id, insert_time, last_access_time)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6);
        "#;

        let (conn, _lock) = self.conn.get_write_conn()?;

        conn.execute(put_sql, params).map_err(|e| {
            let msg;
            let code = if Self::is_exists_error(&e) {
                msg = format!(
                    "insert object relation but already exists: key={:?}, target={}",
                    req.cache_key, req.target_object_id,
                );
                warn!("{}", msg);

                BuckyErrorCode::AlreadyExists
            } else {
                msg = format!(
                    "insert object relation error: key={:?}, target={}, {}",
                    req.cache_key, req.target_object_id, e,
                );
                error!("{}", msg);

                BuckyErrorCode::SqliteError
            };

            BuckyError::new(code, msg)
        })?;

        info!(
            "insert object relation success: key={:?}, target={}",
            req.cache_key, req.target_object_id,
        );

        Ok(())
    }

    pub fn get(
        &self,
        req: &NamedObjectRelationCacheGetRequest,
    ) -> BuckyResult<Option<NamedObjectRelationCacheData>> {
        let object_id = req.cache_key.object_id.to_string();
        let now = bucky_time_now() as i64;
        let relation_type: u8 = req.cache_key.relation_type.into();
        let params = params![&now, &object_id, relation_type, req.cache_key.relation,];

        let update_sql = r#"
            UPDATE cache_object_relation 
            SET last_access_time=?1
            WHERE 
            object_id=?2 AND relation_type=?3 AND relation=?4 
            RETURNING (target_object_id)
        "#;

        /*
        let get_sql = r#"
            SELECT target_object_id
            FROM cache_object_relation 
            WHERE 
            object_id=?0 AND relation_type=?1 AND relation=?2
        "#;
        */
        
        let ret = {
            let (conn, _lock) = self.conn.get_write_conn()?;

            conn.query_row(update_sql, params, |row| {
                Ok(NamedObjectRelationCacheDataRaw::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("get from object relation cache error: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        match ret {
            Some(v) => Ok(Some(v.try_into()?)),
            None => Ok(None),
        }
    }
}

#[async_trait::async_trait]
impl NamedObjectRelationCache for SqliteDBObjectRelationCache {
    async fn put(&self, req: &NamedObjectRelationCachePutRequest) -> BuckyResult<()> {
        Self::put(&self, req)
    }

    async fn get(
        &self,
        req: &NamedObjectRelationCacheGetRequest,
    ) -> BuckyResult<Option<NamedObjectRelationCacheData>> {
        Self::get(&self, req)
    }
}
