use super::super::meta::*;
use super::super::access::*;
use super::data::*;
use super::sql::*;
use cyfs_base::*;
use cyfs_lib::*;

use rusqlite::{named_params, Connection, OptionalExtension, ToSql};
use std::cell::RefCell;
use std::convert::{TryFrom, TryInto};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use thread_local::ThreadLocal;

#[derive(Debug)]
pub struct UpdateObjectMetaRequest<'a> {
    pub object_id: &'a ObjectId,

    pub storage_category: Option<&'a NamedObjectStorageCategory>,
    pub context: Option<&'a String>,
    pub last_access_rpath: Option<&'a String>,
    pub access_string: Option<u32>,
}

impl<'a> UpdateObjectMetaRequest<'a> {
    pub fn is_empty(&self) -> bool {
        self.storage_category.is_none()
            && self.context.is_none()
            && self.last_access_rpath.is_none()
            && self.access_string.is_none()
    }
}

pub(crate) struct SqliteMetaStorage {
    data_dir: PathBuf,
    data_file: PathBuf,

    access: NamedObjecAccessHelper,

    /* SQLite does not support multiple writers. */
    conn: Arc<ThreadLocal<RefCell<Connection>>>,
    conn_rw_lock: RwLock<u32>,
}

impl SqliteMetaStorage {
    pub fn new(root: &Path) -> BuckyResult<Self> {
        let data_file = root.join("meta.db");

        // 需要在开启connection之前调用
        let file_exists = data_file.exists();

        info!(
            "noc sqlite meta db file: {}, exists={}",
            data_file.display(),
            file_exists
        );

        let ret = Self {
            data_dir: root.to_owned(),
            data_file,
            access: NamedObjecAccessHelper::new(),
            conn: Arc::new(ThreadLocal::new()),
            conn_rw_lock: RwLock::new(0),
        };

        if !file_exists {
            if let Err(e) = ret.init_db() {
                error!("init noc sqlite meta db error! now will delete file, {}", e);
                Self::remove_db_file(&ret.data_file, &ret.data_dir);

                return Err(e);
            }
        } else {
            ret.check_and_update()?;
        }

        Ok(ret)
    }

    fn remove_db_file(data_file: &PathBuf, dir: &PathBuf) {
        let tmp_file = dir.join(format!("meta.db,{}", bucky_time_now()));
        if let Err(e) = std::fs::rename(&data_file, &tmp_file) {
            error!(
                "rename meta db file error: {} -> {}, {}",
                data_file.display(),
                tmp_file.display(),
                e
            );
        }
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

        for sql in INIT_NAMEDOBJECT_META_SQL_LIST.iter() {
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
            info!("will update noc meta db: {} -> {}", old, CURRENT_VERSION);

            self.backup_db_file(old)?;

            for version in old + 1..CURRENT_VERSION + 1 {
                Self::update_db(conn.deref_mut(), version)?;

                Self::update_db_version(&conn, version)?;

                assert_eq!(Self::get_db_version(&conn).unwrap(), version);
            }

            info!("update noc meta db success!");
        } else {
            info!(
                "noc meta db version match or newer: db={}, current={}",
                old, CURRENT_VERSION
            );
        }

        Ok(())
    }

    fn backup_db_file(&self, old_version: i32) -> BuckyResult<()> {
        let (hash, _len) = cyfs_base::hash_file_sync(&self.data_file)?;
        let backup_file =
            self.data_file
                .with_extension(format!("{}.{}.db", old_version, hash.to_hex_string()));

        if backup_file.exists() {
            warn!(
                "backup noc meta db file but already exists! file={}",
                backup_file.display()
            );
            return Ok(());
        }

        if let Err(e) = std::fs::copy(&self.data_file, &backup_file) {
            let msg = format!(
                "copy noc meta db file to backup file error! {} -> {}, {}",
                self.data_file.display(),
                backup_file.display(),
                e,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        info!(
            "copy noc meta db file to backup file success! {} -> {}",
            self.data_file.display(),
            backup_file.display()
        );
        Ok(())
    }

    fn update_db(conn: &mut Connection, to_version: i32) -> BuckyResult<()> {
        info!(
            "will exec update noc meta db sqls for version: {}",
            to_version
        );
        if to_version <= 0 || to_version as usize > MAIN_TABLE_UPDATE_LIST.len() {
            error!(
                "invalid noc meta update sql list for version={}",
                to_version
            );
            return Err(BuckyError::from(BuckyErrorCode::SqliteError));
        }

        let tx = conn.transaction().map_err(|e| {
            let msg = format!("noc meta db transaction error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = (|| {
            let index = to_version as usize - 1;
            for sql in &MAIN_TABLE_UPDATE_LIST[index] {
                if sql.is_empty() {
                    break;
                }
                info!("will exec update meta db sql: {}", sql);
                tx.execute_batch(sql).map_err(|e| {
                    let msg = format!("noc meta exec query_row error: sql={}, {}", sql, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::SqliteError, msg)
                })?;
            }
            Ok(())
        })();
        if ret.is_ok() {
            tx.commit().map_err(|e| {
                let msg = format!("commit update transaction error: {}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
            info!("update db to version={} success!", to_version);
        } else {
            tx.rollback().map_err(|e| {
                let msg = format!("rollback update transaction error: {}", e);
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

        info!("current noc meta db version is: {:?}", ret);
        let ret = ret.unwrap_or(0);
        Ok(ret)
    }

    fn update_db_version(conn: &Connection, version: i32) -> BuckyResult<()> {
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

    pub async fn stat(&self) -> BuckyResult<NamedObjectMetaStat> {
        let sql = "SELECT COUNT(*) FROM data_namedobject_meta";
        let ret = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.read().unwrap();

            conn.query_row(&sql, [], |row| {
                let count: i64 = row.get(0).unwrap();
                Ok(count)
            })
            .map_err(|e| {
                let msg = format!("noc meta count objects error! sql={}, {}", sql, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        debug!("noc meta count objects {}", ret);

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

        let stat = NamedObjectMetaStat {
            count: ret as u64,
            storage_size: meta.len(),
        };

        Ok(stat)
    }

    fn insert_new(&self, req: &NamedObjectMetaPutObjectRequest) -> BuckyResult<usize> {
        const INSERT_NEW_SQL: &str = r#"INSERT INTO data_namedobject_meta 
        (   object_id, owner_id, object_type, 
            create_dec_id, insert_time, update_time, 
            object_create_time, object_update_time, object_expired_time,
            author, dec_id, prev, body_prev_version, ref_objs, 
            nonce, difficulty,
            storage_category, context, last_access_time, last_access_rpath, access
        ) VALUES
        (   :object_id, :owner_id, :object_type,
            :create_dec_id, :insert_time, :update_time, 
            :object_create_time, :object_update_time, :object_expired_time,
            :author, :dec_id, :prev, :body_prev_version, :ref_objs, 
            :nonce, :difficulty,
            :storage_category, :context, :last_access_time, :last_access_rpath, :access
        ) "#;

        let last_access_time = bucky_time_now();
        let params = named_params! {
            ":object_id": req.object_id.to_string(),
            ":owner_id": req.owner_id.map(|v| v.to_string()),
            ":create_dec_id": req.source.dec.to_string(),
            ":object_type": req.object_type,

            ":insert_time": req.insert_time,
            ":update_time": req.insert_time,  // Will not update if object_id already exists!

            ":object_create_time": req.object_create_time.unwrap_or(0),
            ":object_update_time": req.object_update_time.unwrap_or(0),
            ":object_expired_time": req.object_expired_time.unwrap_or(0),

            ":author": req.author.as_ref().map(|v| v.as_slice()),
            ":dec_id": req.dec_id.as_ref().map(|v| v.as_slice()),
            ":prev": req.prev.as_ref().map(|v| v.as_slice()),
            ":body_prev_version": req.body_prev_version.as_ref().map(|v| v.as_slice()),
            ":ref_objs": req.ref_objs.as_ref().map(|v| v.to_vec().unwrap()),

            ":nonce": req.nonce.as_ref().map(|v| v.to_be_bytes()),
            ":difficulty": 0,

            ":storage_category": req.storage_category.as_u8(),

            ":context": req.context,

            ":last_access_time": last_access_time,
            ":last_access_rpath": req.last_access_rpath,

            ":access": req.access_string,
        };

        let conn = self.get_conn()?.borrow();
        let _lock = self.conn_rw_lock.write().unwrap();

        let count = conn.execute(INSERT_NEW_SQL, params).map_err(|e| {
            let msg;
            let code = if Self::is_exists_error(&e) {
                msg = format!("insert_new but already exists: {}", req.object_id);
                debug!("{}", msg);

                BuckyErrorCode::AlreadyExists
            } else {
                msg = format!("insert_new error: {} {}", req.object_id, e);
                error!("{}", msg);

                BuckyErrorCode::SqliteError
            };

            BuckyError::new(code, msg)
        })?;

        debug!(
            "insert new to noc success: obj={}, access={}",
            req.object_id,
            AccessString::new(req.access_string)
        );

        Ok(count)
    }

    async fn update(
        &self,
        req: &NamedObjectMetaPutObjectRequest,
    ) -> BuckyResult<NamedObjectMetaPutObjectResponse> {
        debug!("noc meta will update: {}", req);

        let mut retry_count = 0;
        loop {
            // In order to avoid some extreme cases into an infinite loop
            retry_count += 1;
            if retry_count > 16 {
                let msg = format!(
                    "update object extend max retry count! obj={}",
                    req.object_id
                );
                error!("{}", msg);

                break Err(BuckyError::from(msg));
            }

            let ret = self.insert_new(req);
            match ret {
                Ok(count) => {
                    assert_eq!(count, 1);

                    let resp = NamedObjectMetaPutObjectResponse {
                        result: NamedObjectMetaPutObjectResult::Accept,
                        object_update_time: req.object_update_time,
                        object_expired_time: req.object_expired_time,
                    };

                    break Ok(resp);
                }
                Err(e) => match e.code() {
                    BuckyErrorCode::AlreadyExists => {
                        let ret = self.query_update_info(&req.object_id)?;
                        if ret.is_none() {
                            // Maybe been deleted between insert and query
                            continue;
                        }

                        let current_info = ret.unwrap();
                        // info!("noc meta current info: {:?}", current_info);

                        self.access
                            .check_access_with_meta_update_info(
                                &req.object_id,
                                &req.source,
                                &current_info,
                                &current_info.create_dec_id,
                                RequestOpType::Write,
                            )
                            .await?;

                        // Check object_update_time
                        let current_update_time = current_info.object_update_time.unwrap_or(0);
                        let new_update_time = req.object_update_time.unwrap_or(0);

                        if current_update_time >= new_update_time {
                            if current_update_time != new_update_time {
                                let msg = format!("noc meta update object but object's update time is older! obj={}, current={}, new={}", 
                                req.object_id, current_update_time, new_update_time);
                                warn!("{}", msg);
                            } else {
                                let msg = format!("noc meta update object but object's update time is same! obj={}, current={}, new={}", 
                                req.object_id, current_update_time, new_update_time);
                                debug!("{}", msg);
                            }

                            // try update meta
                            let meta_req = UpdateObjectMetaRequest {
                                object_id: &req.object_id,
                                storage_category: Some(&req.storage_category),
                                context: req.context.as_ref(),
                                last_access_rpath: req.last_access_rpath.as_ref(),
                                access_string: Some(req.access_string.clone()),
                            };

                            let count = self.update_existing_meta(meta_req, &current_info)?;
                            if count == 0 {
                                continue;
                            }

                            let resp = NamedObjectMetaPutObjectResponse {
                                result: NamedObjectMetaPutObjectResult::AlreadyExists,
                                object_update_time: current_info.object_update_time,
                                object_expired_time: current_info.object_expired_time,
                            };

                            break Ok(resp);
                        }

                        let count = self.update_existing(req, &current_info)?;
                        if count == 0 {
                            warn!(
                                "noc meta update existing but not found, now will retry! obj={}, incoming object's update_time={:?}, current object's update_time={:?}",
                                req.object_id, req.object_update_time, current_info.object_update_time,
                            );
                            continue;
                        }

                        let resp = NamedObjectMetaPutObjectResponse {
                            result: NamedObjectMetaPutObjectResult::Updated,
                            object_update_time: req.object_update_time,
                            object_expired_time: req.object_expired_time,
                        };

                        break Ok(resp);
                    }
                    _ => {
                        break Err(e);
                    }
                },
            }
        }
    }

    fn update_existing(
        &self,
        req: &NamedObjectMetaPutObjectRequest,
        current_info: &NamedObjectMetaUpdateInfo,
    ) -> BuckyResult<usize> {
        // debug!("noc meta update existing: {}", req);

        const UPDATE_SQL: &str = r#"
        UPDATE data_namedobject_meta SET update_time = :update_time, object_update_time = :object_update_time, 
            context = :context,
            last_access_time = :last_access_time, last_access_rpath = :last_access_rpath,
            body_prev_version = :body_prev_version,
            access = :access
            WHERE object_id = :object_id 
            AND object_update_time = :current_object_update_time 
            AND update_time = :current_update_time 
            AND insert_time = :current_insert_time
        "#;

        let params = named_params! {
            ":update_time": req.insert_time,
            ":object_update_time": req.object_update_time.unwrap_or(0),
            ":context": req.context,
            ":last_access_time": req.insert_time,
            ":last_access_rpath": req.last_access_rpath,
            ":object_id": req.object_id.to_string(),
            ":current_object_update_time": current_info.object_update_time.unwrap_or(0),
            ":current_update_time": current_info.update_time,
            ":current_insert_time": current_info.insert_time,
            ":body_prev_version": req.body_prev_version.as_ref().map(|v| v.as_slice()),
            ":access": req.access_string,
        };

        let count = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.write().unwrap();

            conn.execute(UPDATE_SQL, params).map_err(|e| {
                let msg = format!("noc meta update existing error: {} {}", req.object_id, e);
                error!("{}", msg);

                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        if count > 0 {
            assert_eq!(count, 1);
            info!(
                "noc meta update existsing success: obj={}, update_time={} -> {}",
                req.object_id,
                current_info.update_time,
                current_info.object_update_time.unwrap_or(0),
            );
        } else {
            warn!(
                "noc meta update existsing but not changed: obj={}",
                req.object_id
            );
        }

        Ok(count)
    }

    fn query_update_info(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<NamedObjectMetaUpdateInfo>> {
        const QUERY_UPDATE_SQL: &str = r#"
            SELECT create_dec_id, insert_time, update_time, object_update_time, object_expired_time, access, 
            object_type, object_create_time, owner_id, author, dec_id
            FROM data_namedobject_meta WHERE object_id = :object_id;
        "#;

        let params = named_params! {
            ":object_id" : object_id.to_string(),
        };

        let ret = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.read().unwrap();

            conn.query_row(QUERY_UPDATE_SQL, params, |row| {
                Ok(NamedObjectMetaUpdateInfoRaw::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("query_update_info error: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        match ret {
            Some(v) => Ok(Some(v.try_into()?)),
            None => Ok(None),
        }
    }

    fn query_access_info(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<NamedObjectMetaAccessInfo>> {
        const QUERY_UPDATE_SQL: &str = r#"
            SELECT create_dec_id, access FROM data_namedobject_meta WHERE object_id = :object_id;
        "#;

        let params = named_params! {
            ":object_id" : object_id.to_string(),
        };

        let ret = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.read().unwrap();

            conn.query_row(QUERY_UPDATE_SQL, params, |row| {
                Ok(NamedObjectMetaAccessInfoRaw::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("noc meta query_access_info error: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        match ret {
            Some(v) => Ok(Some(v.try_into()?)),
            None => Ok(None),
        }
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

    async fn get(
        &self,
        req: &NamedObjectMetaGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectMetaData>> {
        match self.get_raw(&req.object_id)? {
            Some(data) => {
                // first check access
                self.access
                    .check_access_with_meta_data(
                        &req.object_id,
                        &req.source,
                        &data,
                        &data.create_dec_id,
                        RequestOpType::Read,
                    )
                    .await?;

                // Update the last access info
                let update_req = NamedObjectMetaUpdateLastAccessRequest {
                    object_id: req.object_id.clone(),
                    last_access_time: bucky_time_now(),
                    last_access_rpath: req.last_access_rpath.clone(),
                };

                let _ = self.update_last_access(&update_req);

                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    fn get_raw(&self, object_id: &ObjectId) -> BuckyResult<Option<NamedObjectMetaData>> {
        const GET_SQL: &'static str = r#"
            SELECT * FROM data_namedobject_meta WHERE object_id = :object_id;
        "#;

        let params = named_params! {
            ":object_id": object_id.to_string(),
        };

        let ret = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.read().unwrap();

            conn.query_row(GET_SQL, params, |row| {
                Ok(NamedObjectMetaDataRaw::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("noc meta get object error: obj={}, {}", object_id, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        match ret {
            Some(v) => {
                let data: NamedObjectMetaData = v.try_into().map_err(|e| {
                    error!("noc meta convert raw data to meta data error: {}", e);
                    e
                })?;
                // debug!("noc meta got object={}, access={}", data.object_id, AccessString::new(data.access_string));
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    fn update_last_access(
        &self,
        req: &NamedObjectMetaUpdateLastAccessRequest,
    ) -> BuckyResult<usize> {
        const UPDATE_SQL: &str = r#"
        UPDATE data_namedobject_meta SET last_access_time = :last_access_time, last_access_rpath = :last_access_rpath 
            WHERE object_id = :object_id 
            AND last_access_time <= :last_access_time
        "#;

        let params = named_params! {
            ":last_access_time": req.last_access_time,
            ":last_access_rpath": req.last_access_rpath,
            ":object_id": req.object_id.to_string(),
        };

        let count = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.write().unwrap();

            conn.execute(UPDATE_SQL, params).map_err(|e| {
                let msg = format!("noc meta update last access error: {} {}", req.object_id, e);
                error!("{}", msg);

                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        if count > 0 {
            assert_eq!(count, 1);
            info!(
                "noc meta update last access success: obj={}, last_access_time={}, last_access_rpath={:?}",
                req.object_id,
                req.last_access_time,
                req.last_access_rpath
            );
        } else {
            warn!(
                "noc meta update last access but not changed: obj={}, last_acecss_time={}",
                req.object_id, req.last_access_time,
            );
        }

        Ok(count)
    }

    async fn delete(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        if req.flags & CYFS_NOC_FLAG_DELETE_WITH_QUERY != 0 {
            self.delete_with_query(req).await
        } else {
            self.delete_only(req).await
        }
    }

    async fn delete_with_query(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        let mut retry_count = 0;
        loop {
            // In order to avoid some extreme cases into an infinite loop
            retry_count += 1;
            if retry_count > 16 {
                let msg = format!(
                    "noc meta delete object extend max retry count! obj={}",
                    req.object_id
                );
                error!("{}", msg);

                break Err(BuckyError::from(msg));
            }

            match self.get_raw(&req.object_id)? {
                Some(data) => {
                    // first check access
                    self.access
                        .check_access_with_meta_data(
                            &req.object_id,
                            &req.source,
                            &data,
                            &data.create_dec_id,
                            RequestOpType::Write,
                        )
                        .await?;

                    let access_info = NamedObjectMetaAccessInfo {
                        create_dec_id: data.create_dec_id.clone(),
                        access_string: data.access_string,
                    };

                    let count = self.try_delete(&req.object_id, &access_info)?;
                    if count > 0 {
                        let resp = NamedObjectMetaDeleteObjectResponse {
                            deleted_count: 1,
                            object: Some(data),
                        };

                        break Ok(resp);
                    } else {
                        warn!("noc meta try delete object but unmatch! now will retry! obj={}, create_dec={}, access={}",
                        req.object_id, access_info.create_dec_id, access_info.access_string,);
                        continue;
                    }
                }
                None => {
                    let resp = NamedObjectMetaDeleteObjectResponse {
                        deleted_count: 0,
                        object: None,
                    };

                    break Ok(resp);
                }
            }
        }
    }

    async fn delete_only(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        // Even if the upper-level req-path permission verification is passed, check whether the dec-id matches
        self.delete_only_with_check_access(req).await
    }

    async fn delete_only_with_check_access(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        let mut retry_count = 0;
        loop {
            // In order to avoid some extreme cases into an infinite loop
            retry_count += 1;
            if retry_count > 16 {
                let msg = format!(
                    "noc meta delete object extend max retry count! obj={}",
                    req.object_id
                );
                error!("{}", msg);

                break Err(BuckyError::from(msg));
            }

            let ret = self.query_update_info(&req.object_id)?;
            if ret.is_none() {
                let resp = NamedObjectMetaDeleteObjectResponse {
                    deleted_count: 0,
                    object: None,
                };

                break Ok(resp);
            }

            let current_info = ret.unwrap();

            // Check permission first
            self.access
                .check_access_with_meta_update_info(
                    &req.object_id,
                    &req.source,
                    &current_info,
                    &current_info.create_dec_id,
                    RequestOpType::Write,
                )
                .await?;

            let access_info = NamedObjectMetaAccessInfo {
                create_dec_id: current_info.create_dec_id.clone(),
                access_string: current_info.access_string,
            };
            let count = self.try_delete(&req.object_id, &access_info)?;
            if count > 0 {
                let resp = NamedObjectMetaDeleteObjectResponse {
                    deleted_count: 1,
                    object: None,
                };

                break Ok(resp);
            } else {
                warn!("noc meta try delete object but unmatch! now will retry! obj={}, create_dec={}, access={}",
                req.object_id, current_info.create_dec_id, current_info.access_string,);
                continue;
            }
        }
    }

    fn try_delete(
        &self,
        object_id: &ObjectId,
        access_info: &NamedObjectMetaAccessInfo,
    ) -> BuckyResult<u32> {
        const DELETE_SQL: &str = r#"
            DELETE FROM data_namedobject_meta WHERE object_id=:object_id AND access = :access AND create_dec_id = :create_dec_id;
        "#;

        let params = named_params! {
            ":object_id": object_id.to_string(),
            ":access": access_info.access_string,
            ":create_dec_id": access_info.create_dec_id.to_string(),
        };

        let count = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.write().unwrap();

            conn.execute(&DELETE_SQL, params).map_err(|e| {
                let msg = format!(
                    "noc meta delete error: obj={}, create_dec={}, err={}",
                    object_id, access_info.create_dec_id, e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        let ret = if count > 0 {
            assert!(count == 1);
            info!(
                "noc meta delete object success! obj={}, create_dec={}",
                object_id, access_info.create_dec_id
            );
            1
        } else {
            info!(
                "noc meta delete object but not found or unmatch! obj={}, create_dec={}, access={}",
                object_id, access_info.create_dec_id, access_info.access_string,
            );
            0
        };

        Ok(ret)
    }

    /*
    fn delete_only_without_check_access(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        let conn = self.get_conn()?.borrow();

        assert!(req.source.is_verified());

        let deleted_count = Self::delete_with_id(&conn, &req.object_id)?;
        let resp = NamedObjectMetaDeleteObjectResponse {
            deleted_count,
            object: None,
        };

        Ok(resp)
    }

    fn delete_with_id(conn: &Connection, object_id: &ObjectId) -> BuckyResult<u32> {
        const DELETE_SQL: &str = r#"
            DELETE FROM data_namedobject_meta WHERE object_id=:object_id;
        "#;

        let params = named_params! {
            ":object_id": object_id.to_string(),
        };

        let count = conn.execute(&DELETE_SQL, params).map_err(|e| {
            let msg = format!("noc meta delete error: obj={},err={}", object_id, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = if count > 0 {
            assert!(count == 1);
            info!("noc meta delete object success! obj={}", object_id,);
            1
        } else {
            info!(
                "noc meta delete object but not found or unmatch! obj={}",
                object_id,
            );
            0
        };

        Ok(ret)
    }
    */

    fn exists(&self, req: &NamedObjectMetaExistsObjectRequest) -> BuckyResult<bool> {
        // FIXME Do you need to add pre-authorization detection?
        const EXISTS_SQL: &str =
            "SELECT EXISTS(SELECT 1 FROM data_namedobject_meta WHERE object_id = :object_id)";

        let params = named_params! {
            ":object_id": req.object_id.to_string(),
        };

        let count = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.read().unwrap();

            conn.query_row(&EXISTS_SQL, params, |row| {
                let count: i64 = row.get(0).unwrap();
                Ok(count)
            })
            .map_err(|e| {
                let msg = format!("noc meta exists error: obj={}, err={}", req.object_id, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        let ret = if count > 0 {
            assert!(count == 1);
            debug!("noc meta exists object success! obj={}", req.object_id);
            true
        } else {
            debug!(
                "noc meta exists object but not found or unmatch! obj={}",
                req.object_id
            );
            false
        };

        Ok(ret)
    }

    async fn update_object_meta(
        &self,
        req: &NamedObjectMetaUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        info!("noc meta will update object meta: {:?}", req);

        if req.is_empty() {
            return Ok(());
        }

        let mut retry_count = 0;
        loop {
            // In order to avoid some extreme cases into an infinite loop
            retry_count += 1;
            if retry_count > 16 {
                let msg = format!(
                    "update object extend max retry count! obj={}",
                    req.object_id
                );
                error!("{}", msg);

                break Err(BuckyError::from(msg));
            }

            let ret = self.query_update_info(&req.object_id)?;
            if ret.is_none() {
                let msg = format!(
                    "noc update object meta but not found! obj={}",
                    req.object_id
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }

            let current_info = ret.unwrap();
            // info!("noc meta current info: {:?}", current_info);

            self.access
                .check_access_with_meta_update_info(
                    &req.object_id,
                    &req.source,
                    &current_info,
                    &current_info.create_dec_id,
                    RequestOpType::Write,
                )
                .await?;

            let meta_req = UpdateObjectMetaRequest {
                object_id: &req.object_id,
                storage_category: req.storage_category.as_ref(),
                context: req.context.as_ref(),
                last_access_rpath: req.last_access_rpath.as_ref(),
                access_string: req.access_string.clone(),
            };

            let ret = self.update_existing_meta(meta_req, &current_info)?;
            if ret > 0 {
                break Ok(());
            }
        }
    }

    fn update_existing_meta<'a>(
        &self,
        req: UpdateObjectMetaRequest<'a>,
        current_info: &NamedObjectMetaUpdateInfo,
    ) -> BuckyResult<usize> {
        trace!("noc meta update existing meta: {:?}", req);

        assert!(!req.is_empty());

        let now = bucky_time_now();
        const UPDATE_SQL: &str = r#"
        UPDATE data_namedobject_meta SET update_time = :update_time, last_access_time = :last_access_time, 
            {}
            WHERE object_id = :object_id
            AND access = :current_access
        "#;

        let mut sqls = vec![];
        let mut params: Vec<(&str, &dyn ToSql)> = vec![];

        let object_id = req.object_id.to_string();
        params.push((":update_time", &now as &dyn ToSql));
        params.push((":last_access_time", &now as &dyn ToSql));
        params.push((":object_id", &object_id as &dyn ToSql));
        params.push((":current_access", &current_info.access_string as &dyn ToSql));

        let storage_category_value;
        if let Some(storage_category) = &req.storage_category {
            storage_category_value = Some(storage_category.as_u8());
            params.push((
                ":storage_category",
                storage_category_value.as_ref().unwrap() as &dyn ToSql,
            ));
            sqls.push("storage_category = :storage_category");
        }
        if let Some(context) = &req.context {
            params.push((":context", context as &dyn ToSql));
            sqls.push("context = :context");
        }
        if let Some(last_access_rpath) = &req.last_access_rpath {
            params.push((":last_access_rpath", last_access_rpath as &dyn ToSql));
            sqls.push("last_access_rpath = :last_access_rpath");
        }
        if let Some(access_string) = &req.access_string {
            params.push((":access", access_string as &dyn ToSql));
            sqls.push("access = :access");
        }

        assert!(sqls.len() > 0);
        let sql = UPDATE_SQL.replace("{}", &sqls.join(","));

        let count = {
            let conn = self.get_conn()?.borrow();
            let _lock = self.conn_rw_lock.write().unwrap();

            conn.execute(&sql, params.as_slice()).map_err(|e| {
                let msg = format!(
                    "noc meta update existing meta error: {} {}",
                    req.object_id, e
                );
                error!("{}", msg);

                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        if count > 0 {
            assert_eq!(count, 1);
            info!("noc meta update existsing meta success: {:?}", req,);
        } else {
            warn!(
                "noc meta update existsing meta but not changed: obj={}",
                req.object_id
            );
        }

        Ok(count)
    }

    async fn check_object_access(
        &self,
        req: &NamedObjectMetaCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        let ret = self.query_update_info(&req.object_id)?;
        if ret.is_none() {
            debug!("noc check object meta but not found! obj={}", req.object_id);
            return Ok(None);
        }

        let current_info = ret.unwrap();
        // info!("noc meta current info: {:?}", current_info);

        self.access
            .check_access_with_meta_update_info(
                &req.object_id,
                &req.source,
                &current_info,
                &current_info.create_dec_id,
                req.required_access,
            )
            .await?;

        Ok(Some(()))
    }
}

#[async_trait::async_trait]
impl NamedObjectMeta for SqliteMetaStorage {
    async fn put_object(
        &self,
        req: &NamedObjectMetaPutObjectRequest,
    ) -> BuckyResult<NamedObjectMetaPutObjectResponse> {
        self.update(req).await
    }

    async fn get_object(
        &self,
        req: &NamedObjectMetaGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectMetaData>> {
        self.get(req).await
    }

    async fn delete_object(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        self.delete(req).await
    }

    async fn exists_object(&self, req: &NamedObjectMetaExistsObjectRequest) -> BuckyResult<bool> {
        self.exists(req)
    }

    async fn update_last_access(
        &self,
        req: &NamedObjectMetaUpdateLastAccessRequest,
    ) -> BuckyResult<bool> {
        match Self::update_last_access(&self, req)? {
            n if n >= 1 => Ok(true),
            _ => Ok(false),
        }
    }

    async fn update_object_meta(
        &self,
        req: &NamedObjectMetaUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        Self::update_object_meta(&self, req).await
    }

    async fn check_object_access(
        &self,
        req: &NamedObjectMetaCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        Self::check_object_access(&self, req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectMetaStat> {
        Self::stat(&self).await
    }

    fn bind_object_meta_access_provider(
        &self,
        object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    ) {
        self.access
            .bind_object_meta_access_provider(object_meta_access_provider)
    }
}
