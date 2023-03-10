use super::file_data::*;
use super::sql::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::SqliteConnectionHolder;

use rusqlite::{params, Connection, OptionalExtension, ToSql};
use std::convert::{TryFrom, TryInto};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct SqliteDBDataCache {
    data_file: PathBuf,
    conn: Arc<SqliteConnectionHolder>,
}

impl SqliteDBDataCache {
    pub fn new(isolate: &str) -> BuckyResult<Self> {
        let dir = cyfs_util::get_named_data_root(isolate);

        if !dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let msg = format!("create ndc dir error! dir={}, err={}", dir.display(), e);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        let data_file = dir.join("data.db");

        // 需要在开启connection之前调用
        let file_exists = data_file.exists();

        info!(
            "ndc sqlite db file: {}, exists={}",
            data_file.display(),
            file_exists
        );

        let ret = Self {
            data_file: data_file.clone(),
            conn: Arc::new(SqliteConnectionHolder::new(data_file)),
        };

        if !file_exists {
            if let Err(e) = ret.init_db() {
                error!("init ndc sqlite db error! now will delete file, {}", e);
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

        for sql in INIT_FILE_SQL_LIST.iter().chain(INIT_CHUNK_SQL_LIST.iter()) {
            info!("will exec: {}", sql);
            conn.execute(&sql, []).map_err(|e| {
                let msg = format!("init ndc table error! sql={}, err={}", sql, e);
                error!("{}", msg);
                BuckyError::from(msg)
            })?;
        }

        info!("init ndc sqlite table success!");

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

    pub fn insert_file(&self, req: &InsertFileRequest) -> BuckyResult<()> {
        let file_id = req.file_id.to_string();
        let file_hash = req.file.desc().content().hash().to_string();
        let len = req.file.desc().content().len();

        let (mut conn, _lock) = self.conn.get_write_conn()?;

        let tx = conn.transaction().map_err(|e| {
            let msg = format!("begin sqlite transation error: {}", e);
            error!("{}", e);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = (|| {
            if let Err(e) = Self::insert_file_main(&tx, req, &file_hash, &file_id, len) {
                if e.code() != BuckyErrorCode::AlreadyExists {
                    return Err(e);
                }

                // TODO 如果已经存在，那么是不是要删除现有的，然后插入新的?
            }

            if let Some(list) = &req.quick_hash {
                Self::insert_file_quick_hash(&tx, list, &file_id, len)?;
            }
            if let Some(list) = &req.dirs {
                if let Err(e) = Self::insert_file_refdirs(&tx, list, &file_id) {
                    if e.code() != BuckyErrorCode::AlreadyExists {
                        return Err(e);
                    }

                    // 如果是已经存在，那么直接忽略
                }
            }

            Ok(())
        })();

        if ret.is_ok() {
            tx.commit().map_err(|e| {
                let msg = format!("commit insert file transaction error: {}", e);
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

            info!(
                "insert file success! file_id={}, hash={}, len={}",
                file_id, file_hash, len
            );
        } else {
            tx.rollback().map_err(|e| {
                let msg = format!("rollback insert file transaction error: {}", e);
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }

        ret
    }

    fn insert_file_main(
        conn: &Connection,
        req: &InsertFileRequest,
        hash: &str,
        file_id: &str,
        len: u64,
    ) -> BuckyResult<()> {
        let owner = req.file.desc().owner().map(|v| v.to_string());

        let now = bucky_time_now() as i64;
        let file_params = params![hash, file_id, len as i64, owner, &now, &now, req.flags,];

        let file_sql = r#"
            INSERT INTO file (hash, file_id, length, owner, insert_time, update_time, flags)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);
        "#;

        conn.execute(file_sql, file_params).map_err(|e| {
            let msg;
            let code = if Self::is_exists_error(&e) {
                msg = format!(
                    "insert file but already exists: hash={}, len={}, file={}",
                    hash, len, file_id
                );
                warn!("{}", msg);

                BuckyErrorCode::AlreadyExists
            } else {
                msg = format!(
                    "insert file error: hash={}, len={}, file={}, {}",
                    hash, len, file_id, e
                );
                error!("{}", msg);

                BuckyErrorCode::SqliteError
            };

            BuckyError::new(code, msg)
        })?;

        info!(
            "insert file success: hash={}, file={}, len={}",
            hash, file_id, len
        );
        Ok(())
    }

    pub fn remove_file(&self, req: &RemoveFileRequest) -> BuckyResult<usize> {
        let file_id = req.file_id.to_string();
        let (mut conn, _lock) = self.conn.get_write_conn()?;

        let tx = conn.transaction().map_err(|e| {
            let msg = format!("begin sqlite transation error: {}", e);
            error!("{}", e);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = (|| {
            let count = Self::delete_file_main(&tx, &file_id)?;
            if count > 0 {
                // FIXME 删除refdir和quickhash失败会不会对最终结果造成影响？
                Self::delete_file_all_refdirs(&tx, &file_id)?;
                Self::delete_file_all_quick_hash(&tx, &file_id)?;
            }
            Ok(count)
        })();

        if ret.is_ok() {
            tx.commit().map_err(|e| {
                let msg = format!("commit remove file transaction error: {}", e);
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

            info!("remove file success! file_id={}", file_id);
        } else {
            tx.rollback().map_err(|e| {
                let msg = format!("rollback remove file transaction error: {}", e);
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }

        ret
    }

    fn delete_file_main(conn: &Connection, file_id: &str) -> BuckyResult<usize> {
        let sql = format!(r#"DELETE FROM file WHERE file_id='{}'"#, file_id);

        let count = conn.execute(&sql, []).map_err(|e| {
            let msg = format!("execute delete file error: sql={}, err={}", sql, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        if count > 0 {
            assert!(count == 1);
            info!(
                "delete file from ndc success! file_id={}, count={}",
                file_id, count
            );
        } else {
            info!("delete file from ndc but not found! file_id={}", file_id,);
        };

        Ok(count)
    }

    fn insert_file_quick_hash(
        conn: &Connection,
        list: &Vec<String>,
        file_id: &str,
        len: u64,
    ) -> BuckyResult<()> {
        for hash in list {
            let params = params![&hash, len as i64, &file_id];
            let sql = r#"
                INSERT INTO file_quick_hash (hash, length, file_id)
                VALUES (?1, ?2, ?3);
            "#;

            // TODO 如果quick_hash+length相同的已经存在，如何处理？
            conn.execute(sql, params).map_err(|e| {
                let msg;
                let code = if Self::is_exists_error(&e) {
                    msg = format!(
                        "insert quick_hash but already exists: quick_hash={}, len={}, file={}",
                        hash, len, file_id
                    );
                    warn!("{}", msg);

                    BuckyErrorCode::AlreadyExists
                } else {
                    msg = format!(
                        "insert quick_hash error: quick_hash={}, len={}, file={}, {}",
                        hash, len, file_id, e
                    );
                    error!("{}", msg);

                    BuckyErrorCode::SqliteError
                };

                BuckyError::new(code, msg)
            })?;

            info!(
                "insert file quick hash: file={}, quick_hash={}, len={}",
                file_id, hash, len
            );
        }

        Ok(())
    }

    fn delete_file_all_quick_hash(conn: &Connection, file_id: &str) -> BuckyResult<usize> {
        let sql = format!(r#"DELETE FROM file_quick_hash WHERE file_id='{}'"#, file_id);

        let count = conn.execute(&sql, []).map_err(|e| {
            let msg = format!(
                "execute delete file all quick hash error: sql={}, err={}",
                sql, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        if count > 0 {
            info!(
                "delete file all quick hash from ndc success! {}, count={}",
                file_id, count
            );
        } else {
            info!(
                "delete file all quick hash from ndc but not found! {}",
                file_id,
            );
        };

        Ok(count)
    }

    fn insert_file_refdirs(
        conn: &Connection,
        list: &Vec<FileDirRef>,
        file_id: &str,
    ) -> BuckyResult<()> {
        for dir_ref in list {
            let dir_id = dir_ref.dir_id.to_string();

            // 确保路径不以/结尾，避免查找时候产生不一致
            let inner_path = dir_ref.inner_path.trim_end_matches('/');
            let params = params![&file_id, &dir_id, &inner_path,];

            let sql = r#"
                INSERT INTO file_dirs (file_id, dir_id, inner_path)
                VALUES (?1, ?2, ?3);
            "#;

            // quick_hash+length为主键但不唯一，所以可能对应多个file_id
            conn.execute(sql, params).map_err(|e| {
                let msg;
                let code = if Self::is_exists_error(&e) {
                    msg = format!("insert file dirs ref but already exists: file_id={}, dir={}, inner_path={}",
                        file_id, dir_id, inner_path);
                    warn!("{}", msg);

                    BuckyErrorCode::AlreadyExists
                } else {
                    msg = format!("insert file dirs ref error: file_id={}, dir={}, inner_path={}, {}",
                    file_id, dir_id, inner_path, e);
                    error!("{}", msg);

                    BuckyErrorCode::SqliteError
                };

                BuckyError::new(code, msg)
            })?;

            info!(
                "insert file ref dirs: file={}, dir={}, inner_path={}",
                file_id, dir_id, inner_path
            );
        }

        Ok(())
    }

    fn delete_file_all_refdirs(conn: &Connection, file_id: &str) -> BuckyResult<usize> {
        let sql = format!(r#"DELETE FROM file_dirs WHERE file_id='{}';"#, file_id);

        let count = conn.execute(&sql, []).map_err(|e| {
            let msg = format!("execute delete file ref dirs error: sql={}, {}", sql, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        if count > 0 {
            info!(
                "delete file ref dirs from ndc success! {}, count={}",
                file_id, count
            );
        } else {
            info!("delete file ref dirs from ndc but not found! {}", file_id);
        };

        Ok(count)
    }

    pub fn get_file_by_hash(
        &self,
        req: &GetFileByHashRequest,
    ) -> BuckyResult<Option<FileCacheData>> {
        let (conn, _lock) = self.conn.get_read_conn()?;
        let data = Self::get_file_main(&conn, &req.hash)?;
        if data.is_none() {
            return Ok(None);
        }

        let mut data = data.unwrap();
        if req.flags != 0 {
            let file_id = data.file_id.to_string();

            if req.flags & NDC_FILE_REQUEST_FLAG_QUICK_HASN != 0 {
                data.quick_hash = Some(Self::get_file_quick_hash(&conn, &file_id)?);
            }

            if req.flags & NDC_FILE_REQUEST_FLAG_REF_DIRS != 0 {
                data.dirs = Some(Self::get_file_ref_dirs(&conn, &file_id)?);
            }
        }

        Ok(Some(data))
    }

    pub fn get_file_by_file_id(
        &self,
        req: &GetFileByFileIdRequest,
    ) -> BuckyResult<Option<FileCacheData>> {
        let file_id = req.file_id.to_string();

        let (conn, _lock) = self.conn.get_read_conn()?;
        Self::get_file_by_file_id_impl(&conn, &file_id, req.flags)
    }

    fn get_file_by_file_id_impl(
        conn: &Connection,
        file_id: &str,
        flags: u32,
    ) -> BuckyResult<Option<FileCacheData>> {
        let data = Self::get_file_main_by_file_id(&conn, file_id)?;
        if data.is_none() {
            return Ok(None);
        }

        let mut data = data.unwrap();
        if flags != 0 {
            let file_id = data.file_id.to_string();

            if flags & NDC_FILE_REQUEST_FLAG_QUICK_HASN != 0 {
                data.quick_hash = Some(Self::get_file_quick_hash(&conn, &file_id)?);
            }

            if flags & NDC_FILE_REQUEST_FLAG_REF_DIRS != 0 {
                data.dirs = Some(Self::get_file_ref_dirs(&conn, &file_id)?);
            }
        }

        Ok(Some(data))
    }

    fn get_file_main(conn: &Connection, hash: &str) -> BuckyResult<Option<FileCacheData>> {
        let sql = r#"
            SELECT * from file where hash=?1;
        "#;

        let ret = conn
            .query_row(sql, params![hash], |row| {
                Ok(SqliteFileCacheData::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("get file by hash from main error: sql={}, err={}", sql, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => {
                let cache_data = v.try_into().map_err(|e| {
                    error!("convert sqlite data to file cache data error: {}", e);
                    e
                })?;
                Ok(Some(cache_data))
            }
            None => Ok(None),
        }
    }

    fn get_file_main_by_file_id(
        conn: &Connection,
        file_id: &str,
    ) -> BuckyResult<Option<FileCacheData>> {
        let sql = r#"
            SELECT * from file where file_id=?1;
        "#;

        let ret = conn
            .query_row(sql, params![file_id], |row| {
                Ok(SqliteFileCacheData::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!(
                    "get file by file_id from main error: sql={}, err={}",
                    sql, e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => {
                let cache_data = v.try_into().map_err(|e| {
                    error!("convert sqlite data to file cache data error: {}", e);
                    e
                })?;
                Ok(Some(cache_data))
            }
            None => Ok(None),
        }
    }

    fn get_file_quick_hash(conn: &Connection, file_id: &str) -> BuckyResult<Vec<String>> {
        let sql = "SELECT hash FROM file_quick_hash WHERE file_id=?1";
        let params = params![file_id];

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select file quick hash error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query(params).map_err(|e| {
            let msg = format!("exec select file quick hash error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<String> = Vec::new();
        while let Some(row) = rows.next()? {
            let quick_hash: String = row.get(0).unwrap();
            result_list.push(quick_hash);
        }

        Ok(result_list)
    }

    fn get_file_ref_dirs(conn: &Connection, file_id: &str) -> BuckyResult<Vec<FileDirRef>> {
        let sql = "SELECT dir_id, inner_path FROM file_dirs WHERE file_id=?1";
        let params = params![file_id];

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select file ref dirs error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query(params).map_err(|e| {
            let msg = format!("exec select file quick hash error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<FileDirRef> = Vec::new();
        while let Some(row) = rows.next()? {
            let dir_id: String = row.get(0).unwrap();
            let inner_path: String = row.get(1).unwrap();

            let dir_id = match DirId::from_str(&dir_id) {
                Err(e) => {
                    error!("convert to dir_id error! {}, {}", dir_id, e);
                    continue;
                }
                Ok(v) => v,
            };

            result_list.push(FileDirRef { dir_id, inner_path });
        }

        Ok(result_list)
    }

    pub fn get_files_by_quick_hash(
        &self,
        req: &GetFileByQuickHashRequest,
    ) -> BuckyResult<Vec<FileCacheData>> {
        let mut file_list = Vec::new();

        let (conn, _lock) = self.conn.get_read_conn()?;

        // 首先从quick_hash表查找对应的file_id列表，可能有多个
        let file_id_list = Self::get_file_list_by_quick_hash(&conn, &req.quick_hash, req.length)?;

        if file_id_list.is_empty() {
            info!(
                "select file by quick hash but not found! quickhash={}, len={}",
                req.quick_hash, req.length
            );
            return Ok(file_list);
        }

        for file_id in file_id_list {
            let ret = Self::get_file_by_file_id_impl(&conn, &file_id, req.flags)?;
            if let Some(data) = ret {
                file_list.push(data);
            } else {
                error!("get file_id from quick_hash but not found in file main table! file_id={}, len={}", file_id, req.length);
            }
        }

        Ok(file_list)
    }

    fn get_file_list_by_quick_hash(
        conn: &Connection,
        hash: &str,
        len: u64,
    ) -> BuckyResult<Vec<String>> {
        let sql;
        let params;
        let len = len as i64;
        if len > 0 {
            sql = "SELECT file_id FROM file_quick_hash WHERE hash=?1 AND length=?2";
            params = params![hash, len].to_owned();
        } else {
            sql = "SELECT file_id FROM file_quick_hash WHERE hash=?1";
            params = params![hash].to_owned();
        }

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select file quick hash error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query(&*params).map_err(|e| {
            let msg = format!("exec select file quick hash error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<String> = Vec::new();
        while let Some(row) = rows.next()? {
            let file_id: String = row.get(0).unwrap();
            result_list.push(file_id);
        }

        Ok(result_list)
    }

    pub fn get_dirs_by_file(&self, req: &GetDirByFileRequest) -> BuckyResult<Vec<FileDirRef>> {
        let file_id = req.file_id.to_string();
        let (conn, _lock) = self.conn.get_read_conn()?;

        Self::get_dirs_by_file_id(&conn, &file_id)
    }

    // 获取和file_id关联的dirs列表
    fn get_dirs_by_file_id(conn: &Connection, file_id: &str) -> BuckyResult<Vec<FileDirRef>> {
        let sql = "SELECT dir_id, inner_path FROM file_dirs WHERE file_id=?1";
        let params = params![file_id];

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select file ref dirs error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query(params).map_err(|e| {
            let msg = format!("exec select file ref dirs error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list = Vec::new();
        while let Some(row) = rows.next()? {
            let dir_id: String = row.get(0).unwrap();
            let inner_path: String = row.get(1).unwrap();

            let dir_id = match DirId::from_str(&dir_id) {
                Ok(v) => v,
                Err(e) => {
                    error!("convert string to dir_id error! str={}, {}", dir_id, e);
                    continue;
                }
            };

            result_list.push(FileDirRef { dir_id, inner_path });
        }

        Ok(result_list)
    }

    pub fn get_files_by_chunk(
        &self,
        req: &GetFileByChunkRequest,
    ) -> BuckyResult<Vec<FileCacheData>> {
        let mut file_list = Vec::new();
        let chunk_id = req.chunk_id.to_string();

        let (conn, _lock) = self.conn.get_read_conn()?;

        // 首先查找chunk关联的所有file_id
        let file_id_list = Self::get_chunk_ref_objects_with_relation(
            &conn,
            &chunk_id,
            ChunkObjectRelation::FileBody,
        )?;

        // 针对每个file_id，依次查询
        for file_id in file_id_list {
            let ret = Self::get_file_by_file_id_impl(&conn, &file_id, req.flags)?;
            match ret {
                Some(data) => {
                    info!(
                        "get file_id from chunk relation: chunk={}, file={}",
                        chunk_id, file_id
                    );
                    file_list.push(data);
                }
                None => {
                    error!("get file_id from chunk relation but file not found in file main table! chunk={}, file={}", chunk_id, file_id);
                }
            }
        }

        Ok(file_list)
    }

    // 获取和chunk_id指定关系的所有对象列表
    fn get_chunk_ref_objects_with_relation(
        conn: &Connection,
        chunk_id: &str,
        relation: ChunkObjectRelation,
    ) -> BuckyResult<Vec<String>> {
        let relation: u8 = relation.into();
        let sql = "SELECT object_id FROM chunk_ref WHERE chunk_id=?1 AND relation=?2";
        let params = params![chunk_id, relation];

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select chunk ref objects by chunk_id error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query(params).map_err(|e| {
            let msg = format!("exec select chunk ref objects by chunk_id error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<String> = Vec::new();
        while let Some(row) = rows.next()? {
            let file_id: String = row.get(0).unwrap();
            result_list.push(file_id);
        }

        Ok(result_list)
    }

    pub fn insert_chunk(&self, req: &InsertChunkRequest) -> BuckyResult<()> {
        let chunk_id = req.chunk_id.to_string();

        let (mut conn, _lock) = self.conn.get_write_conn()?;
        let tx = conn.transaction().map_err(|e| {
            let msg = format!("begin sqlite transation error: {}", e);
            error!("{}", e);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = (|| {
            if let Err(e) = Self::insert_chunk_main(&tx, req, &chunk_id) {
                if e.code() != BuckyErrorCode::AlreadyExists {
                    return Err(e);
                }

                // update state if already exists!
                let req = UpdateChunkStateRequest {
                    chunk_id: req.chunk_id.clone(),
                    current_state: None,
                    state: req.state,
                };

                Self::update_chunk_state_main(&tx, &req)?;
            }

            if let Some(list) = &req.trans_sessions {
                Self::insert_chunk_trans_sessions(&tx, list, &chunk_id)?;
            }

            if let Some(list) = &req.ref_objects {
                Self::insert_chunk_ref_objects(&tx, list, &chunk_id)?;
            }

            Ok(())
        })();

        if ret.is_ok() {
            tx.commit().map_err(|e| {
                let msg = format!("commit insert chunk transaction error: {}", e);
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

            info!(
                "insert chunk success! chunk={}, state={:?}",
                chunk_id, req.state
            );
        } else {
            tx.rollback().map_err(|e| {
                let msg = format!("rollback insert chunk transaction error: {}", e);
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }

        Ok(())
    }

    fn insert_chunk_main(
        conn: &Connection,
        req: &InsertChunkRequest,
        chunk_id: &str,
    ) -> BuckyResult<()> {
        let state: u8 = u8::from(&req.state);

        let now = bucky_time_now() as i64;
        let params = params![chunk_id, &now, &now, 0, state, req.flags,];

        let sql = r#"
            INSERT INTO chunk (chunk_id, insert_time, update_time, last_access_time, state, flags)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6);
        "#;

        conn.execute(sql, params).map_err(|e| {
            let msg;
            let code = if Self::is_exists_error(&e) {
                msg = format!(
                    "insert chunk but already exists: chunk={}, state={:?}, flags={}",
                    chunk_id, req.state, req.flags
                );
                warn!("{}", msg);

                BuckyErrorCode::AlreadyExists
            } else {
                msg = format!(
                    "insert chunk error: chunk={}, state={:?}, flags={}, {}",
                    chunk_id, req.state, req.flags, e
                );
                error!("{}", msg);

                BuckyErrorCode::SqliteError
            };

            BuckyError::new(code, msg)
        })?;

        debug!(
            "insert chunk success: chunk={}, state={:?}, flags={}",
            chunk_id, req.state, req.flags
        );

        Ok(())
    }

    // 插入chunk关联的transation_session
    fn insert_chunk_trans_sessions(
        conn: &Connection,
        list: &Vec<String>,
        chunk_id: &str,
    ) -> BuckyResult<()> {
        for trans_id in list {
            let params = params![&chunk_id, &trans_id,];
            let sql = r#"
                INSERT INTO chunk_trans (chunk_id, trans_id)
                VALUES (?1, ?2);
            "#;

            let ret = conn.execute(sql, params).map_err(|e| {
                let msg;
                let code = if Self::is_exists_error(&e) {
                    msg = format!(
                        "insert chunk_trans but already exists: chunk={}, trans_id={}",
                        chunk_id, trans_id
                    );
                    warn!("{}", msg);

                    BuckyErrorCode::AlreadyExists
                } else {
                    msg = format!(
                        "insert chunk_trans error: chunk={}, trans_id={}, {}",
                        chunk_id, trans_id, e
                    );
                    error!("{}", msg);

                    BuckyErrorCode::SqliteError
                };

                BuckyError::new(code, msg)
            });

            // TODO 如果chunk_id+trans_id相同的组合已经存在，如何处理？这里先忽略，继续下一个
            if ret.is_err() {
                let e = ret.unwrap_err();
                if e.code() == BuckyErrorCode::AlreadyExists {
                    continue;
                }

                // 出错后终止
                return Err(e);
            }
        }

        Ok(())
    }

    // 插入chunk的关联对象列表
    fn insert_chunk_ref_objects(
        conn: &Connection,
        list: &Vec<ChunkObjectRef>,
        chunk_id: &str,
    ) -> BuckyResult<()> {
        for ref_obj in list {
            let object_id = ref_obj.object_id.to_string();
            let relation: u8 = ref_obj.relation.into();

            let params = params![&chunk_id, &object_id, relation,];

            let sql = r#"
                INSERT INTO chunk_ref (chunk_id, object_id, relation)
                VALUES (?1, ?2, ?3);
            "#;

            let ret = conn.execute(sql, params).map_err(|e| {
                let msg;
                let code = if Self::is_exists_error(&e) {
                    msg = format!(
                        "insert chunk_ref but already exists: chunk={}, object={}, relation={:?}",
                        chunk_id, object_id, ref_obj.relation
                    );
                    warn!("{}", msg);

                    BuckyErrorCode::AlreadyExists
                } else {
                    msg = format!(
                        "insert chunk_ref error: chunk={}, object={}, relation={:?}, {}",
                        chunk_id, object_id, ref_obj.relation, e
                    );
                    error!("{}", msg);

                    BuckyErrorCode::SqliteError
                };

                BuckyError::new(code, msg)
            });

            // TODO 如果object+relation相同的组合已经存在，如何处理？这里先忽略，继续下一个
            if ret.is_err() {
                let e = ret.unwrap_err();
                if e.code() == BuckyErrorCode::AlreadyExists {
                    continue;
                }

                // 出错后终止
                return Err(e);
            }
        }

        Ok(())
    }

    pub fn remove_chunk(&self, req: &RemoveChunkRequest) -> BuckyResult<usize> {
        let chunk_id = req.chunk_id.to_string();

        let (mut conn, _lock) = self.conn.get_write_conn()?;
        let tx = conn.transaction().map_err(|e| {
            let msg = format!("begin sqlite transation error: {}", e);
            error!("{}", e);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = (|| {
            let count = Self::delete_chunk_main(&tx, &chunk_id)?;
            if count > 0 {
                // FIXME 删除ref_objects和trans_sessions失败会不会对最终结果造成影响？
                Self::delete_chunk_all_ref_objects(&tx, &chunk_id)?;
                Self::delete_chunk_all_trans_sessions(&tx, &chunk_id)?;
            }

            Ok(count)
        })();

        if ret.is_ok() {
            tx.commit().map_err(|e| {
                let msg = format!(
                    "commit remove chunk transaction error: chunk={}, {}",
                    chunk_id, e
                );
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

            info!("remove chunk success! chunk={}", chunk_id);
        } else {
            tx.rollback().map_err(|e| {
                let msg = format!(
                    "rollback remove chunk transaction error: chunk={}, {}",
                    chunk_id, e
                );
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }

        // FIXME  文件不存在，返回NotFound错误还是Ok(0)?
        ret
    }

    fn delete_chunk_main(conn: &Connection, chunk_id: &str) -> BuckyResult<usize> {
        let sql = format!(r#"DELETE FROM chunk WHERE chunk_id='{}'"#, chunk_id);

        let count = conn.execute(&sql, []).map_err(|e| {
            let msg = format!("execute delete chunk error: sql={}, err={}", sql, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        if count > 0 {
            assert!(count == 1);
            info!("delete chunk from ndc success! {}", chunk_id);
        } else {
            info!("delete chunk from ndc but not found! {}", chunk_id,);
        };

        Ok(count)
    }

    fn delete_chunk_all_trans_sessions(conn: &Connection, chunk_id: &str) -> BuckyResult<usize> {
        let sql = format!(r#"DELETE FROM chunk_trans WHERE chunk_id='{}'"#, chunk_id);

        let count = conn.execute(&sql, []).map_err(|e| {
            let msg = format!(
                "execute delete chunk trans sessions error: sql={}, err={}",
                sql, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        if count > 0 {
            info!(
                "delete chunk trans sessions from ndc success! {}, count={}",
                chunk_id, count
            );
        } else {
            info!(
                "delete chunk trans sessions from ndc but not found! {}",
                chunk_id,
            );
        };

        Ok(count)
    }

    fn delete_chunk_trans_sessions(
        conn: &Connection,
        list: &Vec<String>,
        chunk_id: &str,
    ) -> BuckyResult<usize> {
        let mut total = 0;
        for trans_id in list {
            let sql = format!(
                r#"DELETE FROM chunk_trans WHERE chunk_id='{}' and trans_id='{}'"#,
                chunk_id, trans_id
            );

            let count = conn.execute(&sql, []).map_err(|e| {
                let msg = format!(
                    "execute delete chunk trans sessions error: sql={}, err={}",
                    sql, e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

            if count > 0 {
                assert!(count == 1);
                info!(
                    "delete chunk trans session from ndc success! chunk={}, trans session={}",
                    chunk_id, trans_id,
                );
                total += 1;
            } else {
                info!(
                    "delete chunk trans sessions from ndc but not found! chunk={}, trans session={}",
                    chunk_id, trans_id,
                );
            };
        }

        Ok(total)
    }

    fn delete_chunk_all_ref_objects(conn: &Connection, chunk_id: &str) -> BuckyResult<usize> {
        let sql = format!(r#"DELETE FROM chunk_ref WHERE chunk_id='{}'"#, chunk_id);

        let count = conn.execute(&sql, []).map_err(|e| {
            let msg = format!(
                "execute delete chunk ref objects error: sql={}, err={}",
                sql, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        if count > 0 {
            info!(
                "delete chunk ref objects from ndc success! {}, count={}",
                chunk_id, count
            );
        } else {
            info!(
                "delete chunk ref objects from ndc but not found! {}",
                chunk_id,
            );
        };

        Ok(count)
    }

    fn delete_chunk_ref_objects(
        conn: &Connection,
        list: &Vec<ChunkObjectRef>,
        chunk_id: &str,
    ) -> BuckyResult<usize> {
        let mut total = 0;
        for item in list {
            let object_id = item.object_id.to_string();
            let relation: u8 = item.relation.into();
            let sql = format!(
                r#"DELETE FROM chunk_ref WHERE chunk_id='{}' and object_id='{}' AND relation='{}'"#,
                chunk_id, object_id, relation
            );

            let count = conn.execute(&sql, []).map_err(|e| {
                let msg = format!(
                    "execute delete chunk ref objects error: sql={}, err={}",
                    sql, e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

            if count > 0 {
                assert!(count == 1);
                info!(
                    "delete chunk ref objects from ndc success! chunk={}, object={}, relation={:?}",
                    chunk_id, object_id, item.relation,
                );
                total += 1;
            } else {
                info!(
                    "delete chunk ref objects from ndc but not found! chunk={}, object={}, relation={:?}",
                    chunk_id, object_id, item.relation,
                );
            };
        }

        Ok(total)
    }

    // 更新chunk的最后更新时间
    fn update_chunk_update_time(conn: &Connection, chunk_id: &str) -> BuckyResult<()> {
        let sql = r#"
            UPDATE chunk SET update_time=?1 WHERE chunk_id=?2;
        "#;

        let update_time = bucky_time_now() as i64;
        let params = params![update_time, chunk_id];

        conn.execute(sql, params).map_err(|e| {
            let msg = format!(
                "update chunk update_time error: chunk={}, update_time={}, err={}",
                chunk_id, update_time, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        info!(
            "update chunk update_time success! chunk={}, update_time={}",
            chunk_id, update_time
        );

        Ok(())
    }

    // 更新chunk的最后读取时间
    fn update_chunk_last_access_time(conn: &Connection, chunk_id: &str) -> BuckyResult<()> {
        let sql = r#"
            UPDATE chunk SET last_access_time=?1 WHERE chunk_id=?2;
        "#;

        let last_access_time = bucky_time_now() as i64;
        let params = params![last_access_time, chunk_id];

        conn.execute(sql, params).map_err(|e| {
            let msg = format!(
                "update chunk last_access_time error: chunk={}, last_access_time={}, err={}",
                chunk_id, last_access_time, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        debug!(
            "update chunk last_access_time success! chunk={}, last_access_time={}",
            chunk_id, last_access_time
        );

        Ok(())
    }

    // 更新一组chunk的last_access_time
    fn update_chunk_list_last_access_time(
        conn: &Connection,
        chunk_id_list: &Vec<String>,
    ) -> BuckyResult<()> {
        let query_list: Vec<String> = chunk_id_list
            .iter()
            .map(|v| format!(r#""{}""#, v))
            .collect();
        let query_list = query_list.join(",");

        let last_access_time = bucky_time_now() as i64;

        let sql = format!(
            "UPDATE chunk SET last_access_time={} WHERE chunk_id IN ({});",
            last_access_time, query_list
        );

        conn.execute(&sql, []).map_err(|e| {
            let msg = format!(
                "update chunk last_access_time error: chunk={}, last_access_time={}, err={}",
                query_list, last_access_time, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        info!(
            "update chunk last_access_time success! chunk={}, last_access_time={}",
            query_list, last_access_time
        );

        Ok(())
    }

    // 更新chunk的状态
    fn update_chunk_state(&self, req: &UpdateChunkStateRequest) -> BuckyResult<ChunkState> {
        let (conn, _lock) = self.conn.get_write_conn()?;

        Self::update_chunk_state_main(&conn, req)
    }

    fn update_chunk_state_main(
        conn: &Connection,
        req: &UpdateChunkStateRequest,
    ) -> BuckyResult<ChunkState> {
        let chunk_id = req.chunk_id.to_string();
        let state: u8 = u8::from(&req.state);

        // 首先查询现有的state状态
        let old_state = Self::get_chunk_state(&conn, &chunk_id)?;
        if old_state.is_none() {
            let msg = format!("update chunk state but not found, chunk={}", chunk_id);
            info!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        let old_state = old_state.unwrap();

        let update_time = bucky_time_now() as i64;

        // 根据不同的条件更新
        let count = if let Some(current_state) = &req.current_state {
            if old_state != *current_state {
                let msg = format!(
                    "update chunk state but not match, cur={:?}, expect={:?}",
                    old_state, current_state
                );
                info!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }

            let current_state: u8 = u8::from(current_state);
            let sql = r#"
                UPDATE chunk SET state=?1, update_time=?2 WHERE chunk_id=?3 AND state=?4;
            "#;
            let params = params![state, update_time, chunk_id, current_state];

            conn.execute(sql, params).map_err(|e| {
                let msg = format!(
                    "update chunk state error: chunk={}, state={:?}, err={}",
                    chunk_id, req.state, e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        } else {
            let sql = r#"
                UPDATE chunk SET state=?1, update_time=?2 WHERE chunk_id=?3;
            "#;

            let params = params![state, update_time, chunk_id];

            conn.execute(sql, params).map_err(|e| {
                let msg = format!(
                    "update chunk state error: chunk={}, state={:?}, err={}",
                    chunk_id, req.state, e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        // 可能在此期间发生了竞争？删除或者改变了
        if count == 0 {
            let msg = format!("update chunk state but not found or state unmatch! chunk={}, cur_state={:?}, state={:?}",
                chunk_id, req.current_state, req.state,);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        info!(
            "update chunk state success! chunk={}, cur_state={:?}, state={:?}",
            chunk_id, req.current_state, req.state,
        );

        Ok(old_state)
    }

    fn get_chunk_state(conn: &Connection, chunk_id: &str) -> BuckyResult<Option<ChunkState>> {
        let sql = r#"
            SELECT state FROM chunk WHERE chunk_id=?1;
        "#;

        let ret = conn
            .query_row(sql, params![chunk_id], |row| {
                let state: u8 = row.get(0).unwrap();
                Ok(state)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("get chunk state error: sql={}, err={}", sql, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => {
                let state = ChunkState::try_from(v)?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    pub fn update_chunk_trans_session(
        &self,
        req: &UpdateChunkTransSessionRequest,
    ) -> BuckyResult<()> {
        let chunk_id = req.chunk_id.to_string();

        let (mut conn, _lock) = self.conn.get_write_conn()?;
        let tx = conn.transaction().map_err(|e| {
            let msg = format!("begin sqlite transation error: {}", e);
            error!("{}", e);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = (|| {
            if !req.remove_list.is_empty() {
                Self::delete_chunk_trans_sessions(&tx, &req.remove_list, &chunk_id)?;
            }

            if !req.add_list.is_empty() {
                Self::insert_chunk_trans_sessions(&tx, &req.add_list, &chunk_id)?;
            }

            let _r = Self::update_chunk_update_time(&tx, &chunk_id);

            Ok(())
        })();

        if ret.is_ok() {
            tx.commit().map_err(|e| {
                let msg = format!(
                    "commit update chunk trans sessions error: chunk={}, {}",
                    chunk_id, e
                );
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        } else {
            tx.rollback().map_err(|e| {
                let msg = format!(
                    "rollback update chunk trans sessions error: chunk={}, {}",
                    chunk_id, e
                );
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }

        ret
    }

    pub fn update_chunk_ref_objects(&self, req: &UpdateChunkRefsRequest) -> BuckyResult<()> {
        let chunk_id = req.chunk_id.to_string();

        let (mut conn, _lock) = self.conn.get_write_conn()?;
        let tx = conn.transaction().map_err(|e| {
            let msg = format!("begin sqlite transation error: {}", e);
            error!("{}", e);
            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let ret = (|| {
            if !req.remove_list.is_empty() {
                Self::delete_chunk_ref_objects(&tx, &req.remove_list, &chunk_id)?;
            }

            if !req.add_list.is_empty() {
                Self::insert_chunk_ref_objects(&tx, &req.add_list, &chunk_id)?;
            }

            // 需要更新update_time字段
            let _r = Self::update_chunk_update_time(&tx, &chunk_id);

            Ok(())
        })();

        if ret.is_ok() {
            tx.commit().map_err(|e| {
                let msg = format!(
                    "commit update chunk trans ref objects error: chunk={}, {}",
                    chunk_id, e
                );
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        } else {
            tx.rollback().map_err(|e| {
                let msg = format!(
                    "rollback update chunk trans ref objects error: chunk={}, {}",
                    chunk_id, e
                );
                error!("{}", e);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }

        ret
    }

    pub fn exists_chunks(&self, req: &ExistsChunkRequest) -> BuckyResult<Vec<bool>> {
        let (conn, _lock) = self.conn.get_read_conn()?;

        let states: Vec<String> = req
            .states
            .iter()
            .map(|state| {
                let state: u8 = u8::from(*state);
                state.to_string()
            })
            .collect();
        let states = states.join(",");

        let mut result = Vec::with_capacity(req.chunk_list.len());
        for chunk_id in &req.chunk_list {
            let exists = Self::exists_chunk(&conn, chunk_id, &states)?;
            result.push(exists);
        }

        Ok(result)
    }

    fn exists_chunk(conn: &Connection, chunk_id: &ChunkId, states: &str) -> BuckyResult<bool> {
        let sql = format!(
            r#"
            SELECT EXISTS(SELECT 1 FROM chunk WHERE chunk_id="{}" AND state IN ({}) LIMIT 1);
        "#,
            chunk_id.to_string(),
            states
        );

        let ret = conn
            .query_row(&sql, params![], |row| {
                let exists: bool = row.get(0)?;
                Ok(exists)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("exists chunk by id error: sql={}, err={}", sql, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => Ok(v),
            None => Ok(false),
        }
    }

    pub fn get_chunk(&self, req: &GetChunkRequest) -> BuckyResult<Option<ChunkCacheData>> {
        let chunk_id = req.chunk_id.to_string();

        let (conn, _lock) = self.conn.get_read_conn()?;
        let data = Self::get_chunk_main(&conn, &chunk_id, &req.chunk_id)?;
        if data.is_none() {
            return Ok(None);
        }

        let mut data = data.unwrap();
        if req.flags != 0 {
            if req.flags & NDC_CHUNK_REQUEST_FLAG_TRANS_SESSIONS != 0 {
                data.trans_sessions = Some(Self::get_chunk_trans_sessions_impl(&conn, &chunk_id)?);
            }

            if req.flags & NDC_CHUNK_REQUEST_FLAG_REF_OBJECTS != 0 {
                data.ref_objects = Some(Self::get_chunk_ref_objects_impl(&conn, &chunk_id, &None)?);
            }
        }

        // 更新last_access_time
        let _r = Self::update_chunk_last_access_time(&conn, &chunk_id);

        Ok(Some(data))
    }

    fn get_chunk_main(
        conn: &Connection,
        chunk_id: &str,
        chunk: &ChunkId,
    ) -> BuckyResult<Option<ChunkCacheData>> {
        let sql = r#"
            SELECT insert_time,update_time,last_access_time,state,flags
            FROM chunk WHERE chunk_id=?1;
        "#;

        let ret = conn
            .query_row(sql, params![chunk_id], |row| {
                Ok(SqliteChunkCacheData::try_from(row)?)
            })
            .optional()
            .map_err(|e| {
                let msg = format!("get chunk by id error: sql={}, err={}", sql, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        match ret {
            Some(v) => {
                let cache_data = v.into_chunk_data(chunk.to_owned()).map_err(|e| {
                    error!("convert sqlite data to file cache data error: {}", e);
                    e
                })?;
                Ok(Some(cache_data))
            }
            None => Ok(None),
        }
    }

    pub fn get_chunks(
        &self,
        req: &Vec<GetChunkRequest>,
    ) -> BuckyResult<Vec<Option<ChunkCacheData>>> {
        let (conn, _lock) = self.conn.get_read_conn()?;
        let mut result_list = Vec::new();
        let mut chunk_id_list = Vec::new();
        for req in req {
            let chunk_id = req.chunk_id.to_string();

            let data = Self::get_chunk_main(&conn, &chunk_id, &req.chunk_id)?;
            if data.is_none() {
                result_list.push(None);
                continue;
            }

            let mut data = data.unwrap();
            if req.flags != 0 {
                if req.flags & NDC_CHUNK_REQUEST_FLAG_TRANS_SESSIONS != 0 {
                    data.trans_sessions =
                        Some(Self::get_chunk_trans_sessions_impl(&conn, &chunk_id)?);
                }

                if req.flags & NDC_CHUNK_REQUEST_FLAG_REF_OBJECTS != 0 {
                    data.ref_objects =
                        Some(Self::get_chunk_ref_objects_impl(&conn, &chunk_id, &None)?);
                }
            }

            result_list.push(Some(data));

            chunk_id_list.push(chunk_id);
        }

        // 批量更新last_access_time
        let _r = Self::update_chunk_list_last_access_time(&conn, &chunk_id_list);

        Ok(result_list)
    }

    pub fn get_chunk_trans_sessions(
        &self,
        req: &GetChunkTransSessionsRequest,
    ) -> BuckyResult<Vec<String>> {
        let chunk_id = req.chunk_id.to_string();

        let (conn, _lock) = self.conn.get_read_conn()?;
        Self::get_chunk_trans_sessions_impl(&conn, &chunk_id)
    }

    fn get_chunk_trans_sessions_impl(
        conn: &Connection,
        chunk_id: &str,
    ) -> BuckyResult<Vec<String>> {
        let sql = "SELECT trans_id FROM chunk_trans WHERE chunk_id='?1'";
        let params = params![chunk_id];

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select chunk trans sessions error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query(params).map_err(|e| {
            let msg = format!("exec select chunk trans sessions error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<String> = Vec::new();
        while let Some(row) = rows.next()? {
            let trans_id: String = row.get(0).unwrap();
            result_list.push(trans_id);
        }

        Ok(result_list)
    }

    pub fn get_chunk_ref_objects(
        &self,
        req: &GetChunkRefObjectsRequest,
    ) -> BuckyResult<Vec<ChunkObjectRef>> {
        let chunk_id = req.chunk_id.to_string();

        let (conn, _lock) = self.conn.get_read_conn()?;
        Self::get_chunk_ref_objects_impl(&conn, &chunk_id, &req.relation)
    }

    fn get_chunk_ref_objects_impl(
        conn: &Connection,
        chunk_id: &str,
        relation: &Option<ChunkObjectRelation>,
    ) -> BuckyResult<Vec<ChunkObjectRef>> {
        if let Some(relation) = relation {
            let list =
                Self::get_chunk_ref_objects_with_relation(&conn, &chunk_id, relation.clone())?;

            // 转换到ChunkObjectRef列表
            let mut result_list = Vec::new();
            for id in list {
                let object_id = match ObjectId::from_str(&id) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("convert string field to object_id error: {}, {}", id, e);
                        continue;
                    }
                };

                result_list.push(ChunkObjectRef {
                    object_id,
                    relation: relation.clone(),
                });
            }

            Ok(result_list)
        } else {
            Self::get_chunk_all_ref_objects(&conn, &chunk_id)
        }
    }

    fn get_chunk_all_ref_objects(
        conn: &Connection,
        chunk_id: &str,
    ) -> BuckyResult<Vec<ChunkObjectRef>> {
        let sql = format!(
            "SELECT object_id, relation FROM chunk_ref WHERE chunk_id='{}';",
            chunk_id
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select chunk all ref objects error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query([]).map_err(|e| {
            let msg = format!("exec select chunk all ref objects error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list = Vec::new();
        while let Some(row) = rows.next()? {
            let object_id: String = row.get(0).unwrap();
            let relation: u8 = row.get(1).unwrap();

            let object_id = match ObjectId::from_str(&object_id) {
                Ok(v) => v,
                Err(e) => {
                    error!("convert to object_id error: {}, {}", object_id, e);
                    continue;
                }
            };

            let relation = ChunkObjectRelation::from(relation);

            result_list.push(ChunkObjectRef {
                object_id,
                relation,
            });
        }

        Ok(result_list)
    }

    fn select_chunk(&self, req: &SelectChunkRequest) -> BuckyResult<SelectChunkResponse> {
        let mut querys = Vec::new();

        let mut params: Vec<Box<dyn ToSql>> = Vec::new();
        if let Some(state) = req.filter.state {
            params.push(Box::new(state.as_u8()));

            let query = format!("state=?{}", params.len());
            querys.push(query);
        }

        let sql = if querys.len() > 0 {
            "SELECT chunk_id FROM chunk WHERE ".to_owned() + &querys.join(" AND ")
        } else {
            "SELECT chunk_id FROM chunk ".to_owned()
        };

        // Sort by insert_time, decrease
        let sql = sql + " ORDER BY insert_time DESC ";

        // Add pagination
        let sql = sql
            + &format!(
                " LIMIT {} OFFSET {}",
                req.opt.page_size,
                req.opt.page_size * req.opt.page_index
            );

        info!(
            "will select chunk from ndc: sql={} filter={:?}, opt={:?}",
            sql, req.filter, req.opt
        );

        let (conn, _lock) = self.conn.get_read_conn()?;
        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select chunk sql error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt
            .query(
                params
                    .iter()
                    .map(|item| item.as_ref())
                    .collect::<Vec<&dyn ToSql>>()
                    .as_slice(),
            )
            .map_err(|e| {
                let msg = format!("exec query select chunk error: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

        let mut list = Vec::new();
        while let Some(row) = rows.next()? {
            let chunk_id: String = row.get(0).map_err(|e| {
                let msg = format!("get chunk_id from query row failed! {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;

            let ret = ChunkId::from_str(&chunk_id);
            if ret.is_err() {
                error!("invalid chunk_id str: {}, {}", chunk_id, ret.unwrap_err());
                continue;
            }

            let chunk_id = ret.unwrap();
            let data = SelectChunkData { chunk_id };

            list.push(data);
        }

        let resp = SelectChunkResponse { list };

        Ok(resp)
    }

    async fn stat(&self) -> BuckyResult<NamedDataCacheStat> {
        let sql = "SELECT COUNT(*) FROM chunk";
        let ret = {
            let (conn, _lock) = self.conn.get_read_conn()?;

            conn.query_row(&sql, [], |row| {
                let count: i64 = row.get(0).unwrap();
                Ok(count)
            })
            .map_err(|e| {
                let msg = format!("ndc count chunks error! sql={}, {}", sql, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?
        };

        debug!("ndc count chunks {}", ret);

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

        let stat = NamedDataCacheStat {
            count: ret as u64,
            storage_size: meta.len(),
        };

        Ok(stat)
    }
}

#[async_trait::async_trait]
impl NamedDataCache for SqliteDBDataCache {
    fn clone(&self) -> Box<dyn NamedDataCache> {
        Box::new(Clone::clone(&self as &SqliteDBDataCache)) as Box<dyn NamedDataCache>
    }

    async fn insert_file(&self, req: &InsertFileRequest) -> BuckyResult<()> {
        SqliteDBDataCache::insert_file(&self, req)
    }

    async fn remove_file(&self, req: &RemoveFileRequest) -> BuckyResult<usize> {
        SqliteDBDataCache::remove_file(&self, req)
    }

    async fn file_update_quick_hash(&self, _req: &FileUpdateQuickhashRequest) -> BuckyResult<()> {
        todo!();
    }

    async fn get_file_by_hash(
        &self,
        req: &GetFileByHashRequest,
    ) -> BuckyResult<Option<FileCacheData>> {
        SqliteDBDataCache::get_file_by_hash(&self, req)
    }

    async fn get_file_by_file_id(
        &self,
        req: &GetFileByFileIdRequest,
    ) -> BuckyResult<Option<FileCacheData>> {
        SqliteDBDataCache::get_file_by_file_id(&self, req)
    }

    async fn get_files_by_quick_hash(
        &self,
        req: &GetFileByQuickHashRequest,
    ) -> BuckyResult<Vec<FileCacheData>> {
        SqliteDBDataCache::get_files_by_quick_hash(&self, req)
    }

    async fn get_files_by_chunk(
        &self,
        req: &GetFileByChunkRequest,
    ) -> BuckyResult<Vec<FileCacheData>> {
        SqliteDBDataCache::get_files_by_chunk(&self, req)
    }
    async fn get_dirs_by_file(&self, req: &GetDirByFileRequest) -> BuckyResult<Vec<FileDirRef>> {
        SqliteDBDataCache::get_dirs_by_file(&self, req)
    }

    // chunk相关接口
    async fn insert_chunk(&self, req: &InsertChunkRequest) -> BuckyResult<()> {
        SqliteDBDataCache::insert_chunk(&self, req)
    }

    async fn remove_chunk(&self, req: &RemoveChunkRequest) -> BuckyResult<usize> {
        SqliteDBDataCache::remove_chunk(&self, req)
    }

    async fn update_chunk_state(&self, req: &UpdateChunkStateRequest) -> BuckyResult<ChunkState> {
        SqliteDBDataCache::update_chunk_state(&self, req)
    }

    async fn update_chunk_ref_objects(&self, req: &UpdateChunkRefsRequest) -> BuckyResult<()> {
        SqliteDBDataCache::update_chunk_ref_objects(&self, req)
    }

    async fn exists_chunks(&self, req: &ExistsChunkRequest) -> BuckyResult<Vec<bool>> {
        SqliteDBDataCache::exists_chunks(&self, req)
    }

    async fn get_chunk(&self, req: &GetChunkRequest) -> BuckyResult<Option<ChunkCacheData>> {
        SqliteDBDataCache::get_chunk(&self, req)
    }

    async fn get_chunks(
        &self,
        req: &Vec<GetChunkRequest>,
    ) -> BuckyResult<Vec<Option<ChunkCacheData>>> {
        SqliteDBDataCache::get_chunks(&self, req)
    }

    async fn get_chunk_ref_objects(
        &self,
        req: &GetChunkRefObjectsRequest,
    ) -> BuckyResult<Vec<ChunkObjectRef>> {
        SqliteDBDataCache::get_chunk_ref_objects(&self, req)
    }

    async fn select_chunk(&self, req: &SelectChunkRequest) -> BuckyResult<SelectChunkResponse> {
        Self::select_chunk(&self, req)
    }

    async fn stat(&self) -> BuckyResult<NamedDataCacheStat> {
        Self::stat(&self).await
    }
}
