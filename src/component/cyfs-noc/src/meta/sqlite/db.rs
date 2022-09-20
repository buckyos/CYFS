use super::super::meta::*;
use super::data::*;
use super::sql::*;
use cyfs_base::*;
use cyfs_lib::*;

use rusqlite::{named_params, Connection, OptionalExtension};
use std::cell::RefCell;
use std::convert::{TryFrom, TryInto};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thread_local::ThreadLocal;

#[derive(Clone)]
pub(crate) struct SqliteMetaStorage {
    data_dir: PathBuf,
    data_file: PathBuf,
    conn: Arc<ThreadLocal<RefCell<Connection>>>,
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
            conn: Arc::new(ThreadLocal::new()),
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
            info!("will update noc sqlite db: {} -> {}", old, CURRENT_VERSION);

            for version in old + 1..CURRENT_VERSION + 1 {
                Self::update_db(conn.deref_mut(), version)?;

                Self::udpate_db_version(&conn, version)?;

                assert_eq!(Self::get_db_version(&conn).unwrap(), version);
            }

            info!("update noc meta table success!");
        } else {
            info!(
                "noc meta version match or newer: db={}, current={}",
                old, CURRENT_VERSION
            );
        }

        Ok(())
    }

    fn update_db(conn: &mut Connection, to_version: i32) -> BuckyResult<()> {
        if to_version <= 0 || to_version as usize > MAIN_TABLE_UPDATE_LIST.len() {
            error!(
                "invalid noc meta update sql list for version={}",
                to_version
            );
            return Err(BuckyError::from(BuckyErrorCode::SqliteError));
        }

        let tx = conn.transaction().map_err(|e| {
            let msg = format!("noc meta transaction error: {}", e);
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
                    let msg = format!("noc meta exec query_row error: sql={}, {}", sql, e);
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

        info!("current noc meta db version is: {:?}", ret);
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

    pub async fn stat(&self) -> BuckyResult<NamedObjectMetaStat> {
        let sql = "SELECT COUNT(*) FROM data_namedobject_meta";
        let ret = {
            let conn = self.get_conn()?.borrow();

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
        (object_id, owner_id, create_dec_id, insert_time, update_time, 
            object_update_time, object_expired_time, storage_category, context, last_access_time, last_access_rpath, access) VALUES
            (:object_id, :owner_id, :create_dec_id, :insert_time, :update_time, :object_update_time, 
            :object_expired_time, :storage_category, :context, :last_access_time, :last_access_rpath, :access) "#;

        let last_access_time = bucky_time_now();
        let params = named_params! {
            ":object_id": req.object_id.to_string(),
            ":owner_id": req.owner_id.map(|v| v.to_string()),
            ":create_dec_id": req.source.dec.to_string(),

            ":insert_time": req.insert_time,
            ":update_time": req.insert_time,  // Will not update if object_id already exists!

            ":object_update_time": req.object_update_time.unwrap_or(0),
            ":object_expired_time": req.object_expired_time.unwrap_or(0),

            ":storage_category": req.storage_category.as_u8(),

            ":context": req.context,

            ":last_access_time": last_access_time,
            ":last_access_rpath": req.last_access_rpath,

            ":access": req.access_string,
        };

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

    fn update(
        &self,
        req: &NamedObjectMetaPutObjectRequest,
    ) -> BuckyResult<NamedObjectMetaPutObjectResponse> {
        info!("noc meta will update: {}", req);

        let mut retry_count = 0;
        loop {
            // In order to avoid some extreme cases into an infinite loop
            retry_count += 1;
            if retry_count > 16 {
                let msg = format!(
                    "udpate object extend max retry count! obj={}",
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

                        if !req.source.is_verified() {
                            // Check permission first
                            let mask = req
                                .source
                                .mask(&current_info.create_dec_id, RequestOpType::Write);
                            if current_info.access_string & mask != mask {
                                let msg = format!("noc meta update object but access been rejected! obj={}, access={:#o}, req access={:#o}", 
                                req.object_id, current_info.access_string, mask);
                                warn!("{}", msg);
                                break Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                            }
                        }

                        // Check object_update_time
                        let current_update_time = current_info.object_update_time.unwrap_or(0);
                        let new_update_time = req.object_update_time.unwrap_or(0);

                        if current_update_time >= new_update_time {
                            let msg = format!("noc meta update object but object's update time is same or older! obj={}, current={}, new={}", 
                                req.object_id, current_update_time, new_update_time);
                            warn!("{}", msg);

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
            context = :context, last_access_time = :last_access_time, last_access_rpath = :last_access_rpath 
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
        };

        let conn = self.get_conn()?.borrow();
        let count = conn.execute(UPDATE_SQL, params).map_err(|e| {
            let msg = format!("noc meta update existing error: {} {}", req.object_id, e);
            error!("{}", msg);

            warn!("{}", msg);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

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
            SELECT create_dec_id, insert_time, update_time, object_update_time, object_expired_time, access FROM data_namedobject_meta WHERE object_id = :object_id;
        "#;

        let params = named_params! {
            ":object_id" : object_id.to_string(),
        };

        let conn = self.get_conn()?.borrow();
        let ret = conn
            .query_row(QUERY_UPDATE_SQL, params, |row| {
                Ok(NamedObjectMetaUpdateInfoRaw::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("query_update_info error: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => Ok(Some(v.try_into()?)),
            None => Ok(None),
        }
    }

    fn query_access_info(
        &self,
        conn: &Connection,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<NamedObjectMetaAccessInfo>> {
        const QUERY_UPDATE_SQL: &str = r#"
            SELECT (create_dec_id, access) FROM data_namedobject_meta WHERE object_id = :object_id;
        "#;

        let params = named_params! {
            ":object_id" : object_id.to_string(),
        };

        let ret = conn
            .query_row(QUERY_UPDATE_SQL, params, |row| {
                Ok(NamedObjectMetaAccessInfoRaw::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("noc meta query_access_info error: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

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

    fn get(
        &self,
        req: &NamedObjectMetaGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectMetaData>> {
        let conn = self.get_conn()?.borrow();

        match self.get_raw(&conn, &req.object_id)? {
            Some(data) => {
                if !req.source.is_verified() {
                    // Check permission first
                    let mask = req.source.mask(&data.create_dec_id, RequestOpType::Read);

                    // debug!("get meta data={:?}, access={:o}, mask={:o}", data, data.access_string, mask);

                    if data.access_string & mask != mask {
                        let msg = format!("noc meta get object but access been rejected! obj={}, access={:#o}, req access={:#o}", 
                            req.object_id, data.access_string, mask);
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                    }
                }

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

    fn get_raw(
        &self,
        conn: &Connection,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<NamedObjectMetaData>> {
        const GET_SQL: &'static str = r#"
            SELECT * FROM data_namedobject_meta WHERE object_id=:object_id;
        "#;

        let params = named_params! {
            ":object_id": object_id.to_string(),
        };

        let ret = conn
            .query_row(GET_SQL, params, |row| {
                Ok(NamedObjectMetaDataRaw::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("noc meta get object error: obj={}, {}", object_id, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => {
                let data = v.try_into().map_err(|e| {
                    error!("noc meta convert raw data to meta data error: {}", e);
                    e
                })?;
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

        let conn = self.get_conn()?.borrow();
        let count = conn.execute(UPDATE_SQL, params).map_err(|e| {
            let msg = format!("noc meta update last access error: {} {}", req.object_id, e);
            error!("{}", msg);

            warn!("{}", msg);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

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

    fn delete(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        if req.flags & CYFS_NOC_FLAG_DELETE_WITH_QUERY != 0 {
            self.delete_with_query(req)
        } else {
            self.delete_only(req)
        }
    }

    fn delete_with_query(
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

            let conn = self.get_conn()?.borrow();

            match self.get_raw(&conn, &req.object_id)? {
                Some(data) => {
                    if !req.source.is_verified() {
                        // Check permission first
                        let mask = req.source.mask(&data.create_dec_id, RequestOpType::Write);

                        if data.access_string & mask != mask {
                            let msg = format!("noc meta delete object but access been rejected! obj={}, access={:#o}, req access={:#o}", 
    req.object_id, data.access_string, mask);
                            warn!("{}", msg);
                            break Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                        }
                    }

                    let access_info = NamedObjectMetaAccessInfo {
                        create_dec_id: data.create_dec_id.clone(),
                        access_string: data.access_string,
                    };

                    let count = Self::try_delete(&conn, &req.object_id, &access_info)?;
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

    fn delete_only(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        if req.source.is_verified() {
            self.delete_only_without_check_access(req)
        } else {
            self.delete_only_with_check_access(req)
        }
    }

    fn delete_only_with_check_access(
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

            let conn = self.get_conn()?.borrow();

            let ret = self.query_access_info(&conn, &req.object_id)?;
            if ret.is_none() {
                let resp = NamedObjectMetaDeleteObjectResponse {
                    deleted_count: 0,
                    object: None,
                };

                break Ok(resp);
            }

            let access_info = ret.unwrap();

            // Check permission first
            assert!(!req.source.is_verified());
            let mask = req
                .source
                .mask(&access_info.create_dec_id, RequestOpType::Write);

            if access_info.access_string & mask != mask {
                let msg = format!("noc meta delete object but access been rejected! obj={}, access={:#o}, req access={:#o}", req.object_id, access_info.access_string, mask);
                warn!("{}", msg);
                break Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
            }

            let count = Self::try_delete(&conn, &req.object_id, &access_info)?;
            if count > 0 {
                let resp = NamedObjectMetaDeleteObjectResponse {
                    deleted_count: 1,
                    object: None,
                };

                break Ok(resp);
            } else {
                warn!("noc meta try delete object but unmatch! now will retry! obj={}, create_dec={}, access={}",
                req.object_id, access_info.create_dec_id, access_info.access_string,);
                continue;
            }
        }
    }

    fn try_delete(
        conn: &Connection,
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

        let count = conn.execute(&DELETE_SQL, params).map_err(|e| {
            let msg = format!(
                "noc meta delete error: obj={}, create_dec={}, err={}",
                object_id, access_info.create_dec_id, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

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

    fn exists(&self, req: &NamedObjectMetaExistsObjectRequest) -> BuckyResult<bool> {
        // FIXME Do you need to add pre-authorization detection?
        const EXISTS_SQL: &str =
            "SELECT EXISTS(SELECT 1 FROM data_namedobject_meta WHERE object_id = :object_id)";

        let params = named_params! {
            ":object_id": req.object_id.to_string(),
        };

        let conn = self.get_conn()?.borrow();
        let count = conn
            .query_row(&EXISTS_SQL, params, |row| {
                let count: i64 = row.get(0).unwrap();
                Ok(count)
            })
            .map_err(|e| {
                let msg = format!("noc meta exists error: obj={}, err={}", req.object_id, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

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
}

#[async_trait::async_trait]
impl NamedObjectMeta for SqliteMetaStorage {
    async fn put_object(
        &self,
        req: &NamedObjectMetaPutObjectRequest,
    ) -> BuckyResult<NamedObjectMetaPutObjectResponse> {
        self.update(req)
    }

    async fn get_object(
        &self,
        req: &NamedObjectMetaGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectMetaData>> {
        self.get(req)
    }

    async fn delete_object(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        self.delete(req)
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

    async fn stat(&self) -> BuckyResult<NamedObjectMetaStat> {
        Self::stat(&self).await
    }
}
