use super::sqlite_data::*;
use super::sqlite_sql::*;
use crate::common::*;
use crate::named_object_storage::*;
use async_trait::async_trait;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_lib::*;

use rusqlite::{params, Connection, OptionalExtension, ToSql};
use std::cell::RefCell;
use std::convert::{TryFrom, TryInto};
use std::path::PathBuf;
use std::sync::Arc;
use thread_local::ThreadLocal;

pub(crate) struct SqliteDBCache {
    data_file: PathBuf,
    conn: Arc<ThreadLocal<RefCell<Connection>>>,
    updater: Arc<NOCUpdater>,
}

impl Clone for SqliteDBCache {
    fn clone(&self) -> Self {
        Self {
            data_file: self.data_file.clone(),
            conn: self.conn.clone(),
            updater: self.updater.clone(),
        }
    }
}

impl SqliteDBCache {
    pub fn new(isolate: &str, insert_object_event: InsertObjectEventManager) -> BuckyResult<Self> {
        let dir = cyfs_util::get_cyfs_root_path().join("data");
        let dir = if isolate.len() > 0 {
            dir.join(isolate)
        } else {
            dir
        };
        let dir = dir.join("named-object-cache");

        if !dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let msg = format!("create noc dir error! dir={}, {}", dir.display(), e);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        let data_file = dir.join("object.db");

        // 需要在开启connection之前调用
        let file_exists = data_file.exists();

        info!(
            "noc sqlite db file: {}, exists={}",
            data_file.display(),
            file_exists
        );

        let updater = NOCUpdater::new(insert_object_event);
        let ret = Self {
            data_file,
            conn: Arc::new(ThreadLocal::new()),
            updater: Arc::new(updater),
        };

        if !file_exists {
            if let Err(e) = ret.init_db() {
                error!("init noc sqlite db error! now will delete file, {}", e);
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

    fn get_conn(&self) -> BuckyResult<&RefCell<Connection>> {
        self.conn.get_or_try(|| {
            let ret = self.create_new_conn()?;
            Ok(RefCell::new(ret))
        })
    }

    fn create_new_conn(&self) -> BuckyResult<Connection> {
        let conn = Connection::open(&self.data_file).map_err(|e| {
            let msg = format!("open noc db failed, db={}, {}", self.data_file.display(), e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        // 设置一个30s的锁重试
        if let Err(e) = conn.busy_timeout(std::time::Duration::from_secs(30)) {
            error!("init sqlite busy_timeout error! {}", e);
        }

        Ok(conn)
    }

    fn init_db(&self) -> BuckyResult<()> {
        let conn = self.get_conn()?.borrow();

        for sql in INIT_NOC_SQL_LIST.iter() {
            info!("will exec: {}", sql);
            conn.execute(&sql, []).map_err(|e| {
                let msg = format!("init noc table error! sql={}, {}", sql, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }
        info!("init noc sqlite table success!");

        Ok(())
    }

    fn check_and_update(&self) -> BuckyResult<()> {
        use std::ops::DerefMut;

        let mut conn = self.get_conn()?.borrow_mut();

        let old = Self::get_db_version(&conn)?;
        if old < CURRENT_VERSION {
            info!("will update noc sqlite db: {} -> {}", old, CURRENT_VERSION);

            for version in old + 1..CURRENT_VERSION + 1 {
                Self::update_db(conn.deref_mut(), version)?;

                Self::udpate_db_version(&conn, version)?;

                assert_eq!(Self::get_db_version(&conn).unwrap(), version);
            }

            info!("update noc sqlite table success!");
        } else {
            info!(
                "noc sqlite version match or newer: db={}, current={}",
                old, CURRENT_VERSION
            );
        }

        Ok(())
    }

    fn update_db(conn: &mut Connection, to_version: i32) -> BuckyResult<()> {
        if to_version <= 0 || to_version as usize > MAIN_TABLE_UPDATE_LIST.len() {
            error!("invalid update sql list for version={}", to_version);
            return Err(BuckyError::from(BuckyErrorCode::SqliteError));
        }

        let tx = conn.transaction().map_err(|e| {
            let msg = format!("transaction error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = (|| {
            let index = to_version as usize - 1;
            for sql in &MAIN_TABLE_UPDATE_LIST[index] {
                if sql.is_empty() {
                    break;
                }
                tx.execute(sql, []).map_err(|e| {
                    let msg = format!("exec query_row error: sql={}, {}", sql, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::SqliteError, msg)
                })?;
            }
            Ok(())
        })();
        if ret.is_ok() {
            tx.commit().map_err(|e| {
                let msg = format!("commit transaction error: {}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
            info!("update db to version={} success!", to_version);
        } else {
            tx.rollback().map_err(|e| {
                let msg = format!("rollback transaction error: {}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }
        ret
    }

    fn get_db_version(conn: &Connection) -> BuckyResult<i32> {
        let sql = "PRAGMA USER_VERSION";

        let ret = conn
            .query_row(sql, [], |row| {
                let version: i32 = row.get(0)?;
                Ok(version)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("query_row error: sql={}, {}", sql, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        info!("current db version is: {:?}", ret);
        let ret = ret.unwrap_or(0);
        Ok(ret)
    }

    fn udpate_db_version(conn: &Connection, version: i32) -> BuckyResult<()> {
        let sql = format!("PRAGMA USER_VERSION = {}", version);

        let ret = conn.execute(&sql, []).map_err(|e| {
            let msg = format!("query_row error: sql={}, {}", sql, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        info!(
            "update db version success: version={}, ret={}",
            version, ret
        );
        Ok(())
    }

    pub async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        let sql = "SELECT COUNT(*) FROM noc";
        let ret = {
            let conn = self.get_conn()?.borrow();

            conn.query_row(&sql, [], |row| {
                let count: i64 = row.get(0).unwrap();
                Ok(count)
            })
            .map_err(|e| {
                let msg = format!("count objects error! sql={}, {}", sql, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        debug!("count objects {}", ret);

        let meta = async_std::fs::metadata(&self.data_file)
            .await
            .map_err(|e| {
                let msg = format!(
                    "get metadata of db file error! file={}, err={}",
                    self.data_file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let stat = NamedObjectCacheStat {
            count: ret as u64,
            storage_size: meta.len(),
        };

        Ok(stat)
    }

    pub async fn insert(
        &self,
        req: &ObjectCacheData,
        event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        assert!(req.object_raw.is_some());
        assert!(req.insert_time > 0);

        self.updater.update(self, req, event).await
    }

    pub fn insert_new(&self, req: &ObjectCacheData) -> BuckyResult<usize> {
        assert!(req.object_raw.is_some());
        assert!(req.insert_time > 0);

        let object = req.object.as_ref().unwrap();

        let dec_id = if req.dec_id.is_some() {
            req.dec_id.as_ref().unwrap().to_string()
        } else if let Some(id) = object.dec_id() {
            id.to_string()
        } else {
            "".to_owned()
        };

        let owner_id = if let Some(id) = object.owner() {
            id.to_string()
        } else {
            "".to_owned()
        };

        let author_id = if let Some(id) = object.author() {
            id.to_string()
        } else {
            "".to_owned()
        };

        let params = params![
            req.object_id.to_string(),
            req.protocol.to_string(),
            object.obj_type() as i32,
            object.obj_type_code().to_u16(),
            req.source.to_string(),
            dec_id,
            owner_id,
            author_id,
            req.create_time as i64,
            req.update_time as i64,
            req.insert_time as i64,
            req.rank,
            req.flags,
            &req.object_raw,
        ];

        let conn = self.get_conn()?.borrow();
        let count = conn.execute(INSERT_NEW_SQL, params).map_err(|e| {
            let msg;
            let code = if Self::is_exists_error(&e) {
                msg = format!("insert_new but already exists: {}", req.object_id);
                warn!("{}", msg);

                BuckyErrorCode::AlreadyExists
            } else {
                msg = format!("insert_new error: {} {}", req.object_id, e);
                error!("{}", msg);

                BuckyErrorCode::SqliteError
            };

            BuckyError::new(code, msg)
        })?;

        debug!("insert new to noc success: obj={}", req.object_id);

        Ok(count)
    }

    // 判断是不是相同object_id的项目已经存在
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

    // 更新签名，同时更新insert_time为当前时间
    fn update_signs(&self, req: &ObjectCacheData, insert_time: &u64) -> BuckyResult<usize> {
        assert!(req.object_raw.is_some());
        assert!(req.insert_time > 0);

        let params = params![
            req.object_id.to_string(),
            req.insert_time as i64,
            *insert_time as i64,
            &req.object_raw,
        ];

        let conn = self.get_conn()?.borrow();
        let count = conn.execute(UPDATE_SIGNS_SQL, params).map_err(|e| {
            let msg = format!(
                "update signs to noc table error! obj={}, {}",
                req.object_id, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;
        info!("update signs to noc success: obj={}", req.object_id);

        Ok(count)
    }

    pub fn replace_old(&self, req: &ObjectCacheData, old: &ObjectCacheData) -> BuckyResult<usize> {
        assert!(req.object_raw.is_some());
        assert!(req.insert_time > 0);

        let object = req.object.as_ref().unwrap();

        let dec_id = if req.dec_id.is_some() {
            req.dec_id.as_ref().unwrap().to_string()
        } else if let Some(id) = object.dec_id() {
            id.to_string()
        } else {
            "".to_owned()
        };

        let owner_id = if let Some(id) = object.owner() {
            id.to_string()
        } else {
            "".to_owned()
        };

        let author_id = if let Some(id) = object.author() {
            id.to_string()
        } else {
            "".to_owned()
        };

        let params = params![
            req.object_id.to_string(),
            req.protocol.to_string(),
            object.obj_type() as i32,
            object.obj_type_code().to_u16(),
            req.source.to_string(),
            dec_id,
            owner_id,
            author_id,
            req.create_time as i64,
            req.update_time as i64,
            req.insert_time as i64,
            req.rank,
            req.flags,
            &req.object_raw,
            old.update_time as i64,
        ];

        let conn = self.get_conn()?.borrow();
        let count = conn.execute(UPDATE_SQL, params).map_err(|e| {
            let msg = format!("replace to noc table error! obj={}, {}", req.object_id, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;
        info!("replace old to noc success: obj={}", req.object_id);

        Ok(count)
    }

    pub async fn get(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        match self.try_get(object_id) {
            Ok(Some(obj)) => Ok(Some(obj)),
            Ok(None) => {
                debug!("object not found: {}", object_id);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn try_get(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        let conn = self.get_conn()?.borrow();
        Self::try_get_with_conn(&conn, object_id)
    }

    fn try_get_with_conn(
        conn: &Connection,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectCacheData>> {
        let sql = r#"
            SELECT * FROM noc WHERE object_id=?1;
        "#;

        let ret = conn
            .query_row(sql, params![object_id.to_string()], |row| {
                Ok(SqliteObjectCacheData::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("query_row error: sql={}, {}", sql, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => {
                let cache_data = v.try_into().map_err(|e| {
                    error!("convert sqlite data to object cache data error: {}", e);
                    e
                })?;
                Ok(Some(cache_data))
            }
            None => Ok(None),
        }
    }

    fn delete(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult> {
        let conn = self.get_conn()?.borrow();

        if req.flags & CYFS_REQUEST_FLAG_DELETE_WITH_QUERY != 0 {
            let ret = Self::try_get_with_conn(&conn, &req.object_id)?;
            if ret.is_some() {
                let deleted_count = Self::try_delete(&conn, &req.object_id)?;
                assert!(deleted_count == 1);
                Ok(NamedObjectCacheDeleteObjectResult {
                    deleted_count: 1,
                    object: ret,
                })
            } else {
                Ok(NamedObjectCacheDeleteObjectResult {
                    deleted_count: 0,
                    object: None,
                })
            }
        } else {
            let deleted_count = Self::try_delete(&conn, &req.object_id)?;
            let ret = NamedObjectCacheDeleteObjectResult {
                deleted_count,
                object: None,
            };
            Ok(ret)
        }
    }

    fn try_delete(conn: &Connection, object_id: &ObjectId) -> BuckyResult<u32> {
        let sql = format!(
            r#"
            DELETE FROM noc WHERE object_id='{}';
        "#,
            object_id.to_string()
        );

        let count = conn.execute(&sql, []).map_err(|e| {
            let msg = format!("execute delete error: sql={}, err={}", sql, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = if count > 0 {
            assert!(count == 1);
            info!("delete object from noc success! {}", object_id.to_string());
            1
        } else {
            info!(
                "delete object from noc but not found! {}",
                object_id.to_string()
            );
            0
        };

        Ok(ret)
    }

    fn timerange_query(
        name: &str,
        time_range: &Option<NamedObjectCacheSelectObjectTimeRange>,
        params: &mut Vec<Box<dyn ToSql>>,
        querys: &mut Vec<String>,
    ) {
        if time_range.is_some() {
            let time_range = time_range.as_ref().unwrap();
            if time_range.begin.is_some() {
                let tm = *time_range.begin.as_ref().unwrap() as i64;
                params.push(Box::new(tm));

                let query = format!("{}>=?{}", name, params.len());
                querys.push(query);
            }

            if time_range.end.is_some() {
                let tm = *time_range.end.as_ref().unwrap() as i64;
                params.push(Box::new(tm));

                let query = format!("{}<?{}", name, params.len());
                querys.push(query);
            }
        }
    }

    fn object_id_query<T>(
        name: &str,
        id: &Option<T>,
        params: &mut Vec<Box<dyn ToSql>>,
        querys: &mut Vec<String>,
    ) where
        T: ToString,
    {
        if id.is_some() {
            let id = id.as_ref().unwrap().to_string();
            params.push(Box::new(id));

            let query = format!("{}=?{}", name, params.len());
            querys.push(query);
        }
    }

    pub async fn select(
        &self,
        filter: &NamedObjectCacheSelectObjectFilter,
        opt: Option<&NamedObjectCacheSelectObjectOption>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        let mut querys = Vec::new();

        let mut params: Vec<Box<dyn ToSql>> = Vec::new();
        if filter.obj_type.is_some() {
            let obj_type = *filter.obj_type.as_ref().unwrap() as i32;
            params.push(Box::new(obj_type));

            let query = format!("object_type=?{}", params.len());
            querys.push(query);
        }

        if filter.obj_type_code.is_some() {
            let obj_type_code = filter.obj_type_code.as_ref().unwrap().to_u16() as i16;
            params.push(Box::new(obj_type_code));

            let query = format!("object_type_code=?{}", params.len());
            querys.push(query);
        }

        // dec/owner/authod
        Self::object_id_query("dec_id", &filter.dec_id, &mut params, &mut querys);
        Self::object_id_query("owner_id", &filter.owner_id, &mut params, &mut querys);
        Self::object_id_query("author_id", &filter.author_id, &mut params, &mut querys);

        Self::timerange_query("create_time", &filter.create_time, &mut params, &mut querys);
        Self::timerange_query("update_time", &filter.update_time, &mut params, &mut querys);
        Self::timerange_query("insert_time", &filter.insert_time, &mut params, &mut querys);

        let sql = if querys.len() > 0 {
            "SELECT * FROM noc WHERE ".to_owned() + &querys.join(" AND ")
        } else {
            "SELECT * FROM noc ".to_owned()
        };

        let opt = if opt.is_some() {
            opt.unwrap().to_owned()
        } else {
            NamedObjectCacheSelectObjectOption::default()
        };

        // 以insert_time排序，递减
        let sql = sql + " ORDER BY insert_time DESC ";

        // 添加分页
        let sql = sql
            + &format!(
                " LIMIT {} OFFSET {}",
                opt.page_size,
                opt.page_size * opt.page_index
            );

        info!(
            "will exec select: sql={} filter={:?}, opt={:?}",
            sql, filter, opt
        );

        let conn = self.get_conn()?.borrow();
        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query(params.iter().map(|item| item.as_ref()).collect::<Vec<&dyn ToSql>>().as_slice()).map_err(|e| {
            let msg = format!("exec query error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<ObjectCacheData> = Vec::new();
        while let Some(row) = rows.next()? {
            let raw_data = match SqliteObjectCacheData::try_from(row) {
                Ok(v) => v,
                Err(e) => {
                    error!("decode raw data from row error: {}", e);
                    continue;
                }
            };

            match raw_data.try_into() {
                Ok(cache_data) => {
                    result_list.push(cache_data);
                }
                Err(e) => {
                    error!("convert sqlite data to object cache data error: {}", e);
                }
            }
        }

        Ok(result_list)
    }

    // 判断一组对象是否存在，存在的话更新zone_seq
    async fn diff_objects(&self, list: &Vec<SyncObjectData>) -> BuckyResult<Vec<bool>> {
        let sql = r#"
            UPDATE noc SET zone_seq=?1 WHERE object_id=?2 AND update_time=?3;
        "#;

        let mut result = Vec::with_capacity(list.len());

        let conn = self.get_conn()?.borrow();
        for item in list {
            let count = conn
                .execute(
                    sql,
                    params![
                        Box::new(item.seq.to_owned() as i64),
                        item.object_id.to_string(),
                        Box::new(item.update_time.to_owned() as i64),
                    ],
                )
                .map_err(|e| {
                    let msg = format!("diff objects query error: sql={}, {}", sql, e);
                    error!("{}", msg);

                    BuckyError::new(BuckyErrorCode::SqliteError, msg)
                })?;

            result.push(count > 0);
        }

        Ok(result)
    }

    async fn query_object(&self, object_id: &ObjectId, update_time: &u64) -> BuckyResult<bool> {
        let sql = r#"
            SELECT * FROM noc WHERE object_id=?1 AND update_time=?2;
        "#;

        let ret = self
            .get_conn()?
            .borrow()
            .query_row(
                sql,
                params![
                    object_id.to_string(),
                    Box::new(update_time.to_owned() as i64)
                ],
                |_row| Ok(true),
            )
            .optional()
            .map_err(|e| {
                let msg = format!("query_row error: sql={}, {}", sql, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => Ok(v),
            None => Ok(false),
        }
    }

    fn get_latest_seq(&self) -> BuckyResult<u64> {
        let sql = format!(
            "SELECT insert_time FROM noc WHERE rank>={} ORDER BY insert_time DESC LIMIT 1",
            OBJECT_RANK_SYNC_LEVEL
        );

        info!("will exec get_latest_seq: sql={}", sql);
        let conn = self.get_conn()?.borrow();
        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare get_latest_seq error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query([]).map_err(|e| {
            let msg = format!("exec query error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result: u64 = 0;
        if let Some(row) = rows.next()? {
            match row.get::<usize, i64>(0) {
                Ok(v) => result = v as u64,
                Err(e) => {
                    let msg = format!("decode insert_time from row error: {}", e);
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
                }
            }
        }

        Ok(result)
    }

    fn list_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        page_index: u16,
        page_size: u16,
    ) -> BuckyResult<Vec<SyncObjectData>> {
        let sql = format!("SELECT object_id,insert_time,update_time FROM noc WHERE insert_time>={} AND insert_time<={} AND rank>={} ORDER BY insert_time ASC LIMIT {} OFFSET {}",
            begin_seq, end_seq, OBJECT_RANK_SYNC_LEVEL, page_size, page_size * page_index);

        info!("will exec list_objects: sql={}", sql);

        let conn = self.get_conn()?.borrow();
        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare list_objects error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query([]).map_err(|e| {
            let msg = format!("exec list_objects error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<SyncObjectData> = Vec::new();
        while let Some(row) = rows.next()? {
            let raw_data = match SqliteSyncObjectData::try_from(row) {
                Ok(v) => v,
                Err(e) => {
                    error!("decode raw sync data from row error: {}", e);
                    continue;
                }
            };

            match raw_data.try_into() {
                Ok(sync_data) => {
                    result_list.push(sync_data);
                }
                Err(e) => {
                    error!("convert sqlite data to sync object data error: {}", e);
                }
            }
        }

        Ok(result_list)
    }

    fn get_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        list: &Vec<ObjectId>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        let query_list: Vec<String> = list
            .iter()
            .map(|v| format!(r#""{}""#, v.to_string()))
            .collect();
        let query_list = query_list.join(",");

        let sql = format!(
            "SELECT * FROM noc WHERE insert_time>={} AND insert_time<={} AND object_id IN ({}) ORDER BY insert_time ASC",
            begin_seq, end_seq, query_list,
        );

        debug!("will exec get_objects: sql={}", sql);

        let conn = self.get_conn()?.borrow();
        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare get_objects error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query([]).map_err(|e| {
            let msg = format!("exec get_objects error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<ObjectCacheData> = Vec::new();
        while let Some(row) = rows.next()? {
            let raw_data = match SqliteObjectCacheData::try_from(row) {
                Ok(v) => v,
                Err(e) => {
                    error!("decode raw data from row error: {}", e);
                    continue;
                }
            };

            match raw_data.try_into() {
                Ok(cache_data) => {
                    result_list.push(cache_data);
                }
                Err(e) => {
                    error!("convert sqlite data to object cache data error: {}", e);
                }
            }
        }

        Ok(result_list)
    }
}

#[async_trait]
impl NOCUpdaterProvider for SqliteDBCache {
    async fn try_get(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        SqliteDBCache::try_get(&self, object_id)
    }

    async fn update_signs(&self, req: &ObjectCacheData, insert_time: &u64) -> BuckyResult<usize> {
        SqliteDBCache::update_signs(&self, req, insert_time)
    }

    async fn replace_old(
        &self,
        req: &ObjectCacheData,
        old: &ObjectCacheData,
    ) -> BuckyResult<usize> {
        SqliteDBCache::replace_old(&self, req, old)
    }

    async fn insert_new(&self, req: &ObjectCacheData) -> BuckyResult<usize> {
        SqliteDBCache::insert_new(&self, req)
    }
}

#[async_trait]
impl NamedObjectStorage for SqliteDBCache {
    async fn insert_object(
        &self,
        obj_info: &ObjectCacheData,
        event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        self.insert(obj_info, event).await
    }

    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        self.get(object_id).await
    }

    async fn select_object(
        &self,
        filter: &NamedObjectCacheSelectObjectFilter,
        opt: Option<&NamedObjectCacheSelectObjectOption>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        self.select(filter, opt).await
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult> {
        self.delete(req)
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        self.stat().await
    }

    fn sync_server(&self) -> Option<Box<dyn NamedObjectCacheSyncServer>> {
        Some(Box::new(Clone::clone(&self as &SqliteDBCache)))
    }

    fn sync_client(&self) -> Option<Box<dyn NamedObjectCacheSyncClient>> {
        Some(Box::new(Clone::clone(&self as &SqliteDBCache)))
    }

    fn clone(&self) -> Box<dyn NamedObjectStorage> {
        Box::new(Clone::clone(&self as &SqliteDBCache)) as Box<dyn NamedObjectStorage>
    }
}

#[async_trait]
impl NamedObjectCacheSyncClient for SqliteDBCache {
    async fn query_object(&self, object_id: &ObjectId, update_time: &u64) -> BuckyResult<bool> {
        SqliteDBCache::query_object(&self, object_id, update_time).await
    }

    async fn diff_objects(&self, list: &Vec<SyncObjectData>) -> BuckyResult<Vec<bool>> {
        SqliteDBCache::diff_objects(&self, list).await
    }
}

#[async_trait]
impl NamedObjectCacheSyncServer for SqliteDBCache {
    // 获取当前的最新的seq
    async fn get_latest_seq(&self) -> BuckyResult<u64> {
        SqliteDBCache::get_latest_seq(&self)
    }

    // 查询指定的同步列表
    async fn list_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        page_index: u16,
        page_size: u16,
    ) -> BuckyResult<Vec<SyncObjectData>> {
        SqliteDBCache::list_objects(&self, begin_seq, end_seq, page_index, page_size)
    }

    async fn get_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        list: &Vec<ObjectId>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        SqliteDBCache::get_objects(&self, begin_seq, end_seq, list)
    }
}
