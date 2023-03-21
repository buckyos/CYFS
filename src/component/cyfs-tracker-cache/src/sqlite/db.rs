use super::data::*;
use super::sql::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::SqliteConnectionHolder;

use rusqlite::{params, ToSql};
use std::convert::{TryFrom, TryInto};
use std::path::PathBuf;
use std::sync::Arc;


#[derive(Clone)]
pub(crate) struct SqliteDBDataCache {
    data_file: PathBuf,
    conn: Arc<SqliteConnectionHolder>,
}

impl SqliteDBDataCache {
    pub fn new(isolate: &str) -> BuckyResult<Self> {
        let dir = cyfs_util::get_cyfs_root_path().join("data");
        let dir = if isolate.len() > 0 {
            dir.join(isolate)
        } else {
            dir
        };
        let dir = dir.join("tracker-cache");

        if !dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let msg = format!(
                    "create tracker cache dir error! dir={}, err={}",
                    dir.display(),
                    e
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        let data_file = dir.join("data.db");

        // 需要在开启connection之前调用
        let file_exists = data_file.exists();

        info!(
            "tracker cache sqlite db file: {}, exists={}",
            data_file.display(),
            file_exists
        );

        let ret = Self {
            data_file: data_file.clone(),
            conn: Arc::new(SqliteConnectionHolder::new(data_file)),
        };

        if !file_exists {
            if let Err(e) = ret.init_db() {
                error!("init tracker cache db error! now will delete file, {}", e);
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

        for sql in &INIT_TRACKER_SQL_LIST {
            info!("will exec: {}", sql);
            conn.execute(&sql, []).map_err(|e| {
                let msg = format!("init tracker table error! sql={}, err={}", sql, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            })?;
        }

        info!("init tracker sqlite table success!");

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

    pub fn add_position(&self, req: &AddTrackerPositonRequest) -> BuckyResult<()> {
        let (pos_type, pos): (u8, String) = req.pos.clone().into();
        let direction: u8 = req.direction.into();

        let (conn, _lock) = self.conn.get_write_conn()?;

        let now = bucky_time_now() as i64;
        let file_params = params![req.id, pos, pos_type, direction, &now, &now, req.flags,];

        let file_sql = r#"
            INSERT INTO tracker (id, pos, pos_type, direction, insert_time, update_time, flags)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);
        "#;

        conn.execute(file_sql, file_params).map_err(|e| {
            let msg;
            let code = if Self::is_exists_error(&e) {
                msg = format!(
                    "insert pos but already exists: id={}, pos={:?}, direction={:?}",
                    req.id, req.pos, req.direction,
                );
                warn!("{}", msg);

                BuckyErrorCode::AlreadyExists
            } else {
                msg = format!(
                    "insert pos error: id={}, pos={:?}, direction={:?}, {}",
                    req.id, req.pos, req.direction, e
                );
                error!("{}", msg);

                BuckyErrorCode::SqliteError
            };

            BuckyError::new(code, msg)
        })?;

        info!(
            "insert pos success: id={}, pos={:?}, direction={:?}",
            req.id, req.pos, req.direction,
        );

        Ok(())
    }

    pub fn remove_position(&self, req: &RemoveTrackerPositionRequest) -> BuckyResult<usize> {
        let sql = r#"DELETE FROM tracker WHERE "#;

        let mut querys = Vec::new();
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();

        // id
        params.push(Box::new(&req.id));
        let query = format!("id=?{}", params.len());
        querys.push(query);

        // direction
        if let Some(direction) = req.direction {
            let direction: u8 = direction.clone().into();
            params.push(Box::new(direction));

            let query = format!("direction=?{}", params.len());
            querys.push(query);
        }

        // pos
        if let Some(pos) = &req.pos {
            let (pos_type, pos): (u8, String) = pos.clone().into();
            params.push(Box::new(pos_type));

            let query = format!("pos_type=?{}", params.len());
            querys.push(query);

            params.push(Box::new(pos));
            let query = format!("pos=?{}", params.len());
            querys.push(query);
        }

        let sql = sql.to_owned() + &querys.join(" AND ");

        debug!("will exec delete tracker sql: {}", sql);

        let (conn, _lock) = self.conn.get_write_conn()?;

        let count = conn.execute(&sql, params.iter().map(|item| item.as_ref()).collect::<Vec<&dyn ToSql>>().as_slice()).map_err(|e| {
            let msg = format!("execute delete pos error: sql={}, err={}", sql, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        info!(
            "delete pos from tracker cache success! id={}, pos={:?}, direction={:?}, count={}",
            req.id, req.pos, req.direction, count
        );

        Ok(count)
    }

    pub fn get_position(
        &self,
        req: &GetTrackerPositionRequest,
    ) -> BuckyResult<Vec<TrackerPositionCacheData>> {
        let sql = r#"
            SELECT * from tracker WHERE
        "#;

        let mut querys = Vec::new();
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();

        // id
        params.push(Box::new(&req.id));
        let query = format!("id=?{}", params.len());
        querys.push(query);

        // direction
        if let Some(direction) = req.direction {
            let direction: u8 = direction.clone().into();
            params.push(Box::new(direction));

            let query = format!("direction=?{}", params.len());
            querys.push(query);
        }

        let sql = sql.to_owned() + &querys.join(" AND ");
        let sql = sql + " ORDER BY update_time DESC";

        let (conn, _lock) = self.conn.get_read_conn()?;
        let mut stmt = conn.prepare(&sql).map_err(|e| {
            let msg = format!("prepare select pos error: sql={}, {}", sql, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut rows = stmt.query(params.iter().map(|item| item.as_ref()).collect::<Vec<&dyn ToSql>>().as_slice()).map_err(|e| {
            let msg = format!("exec select query error: sql={}, {}", sql, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        let mut result_list: Vec<TrackerPositionCacheData> = Vec::new();
        while let Some(row) = rows.next()? {
            let raw_data = match SqlitePostionCacheData::try_from(row) {
                Ok(v) => v,
                Err(e) => {
                    error!("decode raw pos data error: {}", e);
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

#[async_trait::async_trait]
impl TrackerCache for SqliteDBDataCache {
    fn clone(&self) -> Box<dyn TrackerCache> {
        Box::new(Clone::clone(&self as &SqliteDBDataCache)) as Box<dyn TrackerCache>
    }

    async fn add_position(&self, req: &AddTrackerPositonRequest) -> BuckyResult<()> {
        SqliteDBDataCache::add_position(&self, req)
    }
    async fn remove_position(&self, req: &RemoveTrackerPositionRequest) -> BuckyResult<usize> {
        SqliteDBDataCache::remove_position(&self, req)
    }

    async fn get_position(
        &self,
        req: &GetTrackerPositionRequest,
    ) -> BuckyResult<Vec<TrackerPositionCacheData>> {
        SqliteDBDataCache::get_position(&self, req)
    }
}
