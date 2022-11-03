use crate::storage::{Storage, map_sql_err, MetaStat, Period};
use cyfs_base::*;
use sqlx::sqlite::{SqlitePoolOptions, SqliteJournalMode, SqliteConnectOptions, SqliteRow};
use sqlx::{Pool, Sqlite, Row, ConnectOptions};
use std::path::Path;
use std::time::Duration;
use log::*;
use async_trait::async_trait;
use once_cell::sync::OnceCell;

const GET_OBJ_DESC_NUM: &str = r#"SELECT count(*) from device_stat where obj_type = ?1"#;
const GET_OBJ_ADD_DESC_NUM: &str = r#"SELECT count(*) from device_stat where obj_type = ?1 and create_time >= ?2 and create_time <= ?3"#;
const GET_OBJ_ACTIVE_DESC_NUM: &str = r#"SELECT count(*) from device_stat where obj_type = ?1 and update_time >= ?2 and update_time <= ?3"#;
const GET_META_OBJ_NUM: &str = r#"SELECT id, success, failed from meta_object_stat where create_time >= ?1 and create_time <= ?2"#;
const GET_META_API_NUM: &str = r#"SELECT id, success, failed from meta_api_stat"#;
pub struct SqliteStorage {
    pool: OnceCell<Pool<Sqlite>>,
}

impl SqliteStorage {
    pub(crate) fn new() -> Self {
        Self {
            pool: OnceCell::new(),
        }
    }

    fn metainfo_from_row(&self, row: &SqliteRow) -> MetaStat {
        let id: String = row.get("id");
        let success: i64 = row.get("success");
        let failed: i64 = row.get("failed");
        MetaStat {
            id,
            success: success as u64,
            failed: failed as u64,
        }
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn open(&mut self, db_path: &str) -> BuckyResult<()> {
        let database = Path::new(db_path).join("archive_db");
        info!("database: {}", database.display());
        let mut options = SqliteConnectOptions::new().filename(database.as_path())
            .journal_mode(SqliteJournalMode::Memory).busy_timeout(Duration::new(10, 0));
        options.log_statements(LevelFilter::Off);
        let pool = SqlitePoolOptions::new().max_connections(10).connect_with(options).await.map_err(map_sql_err)?;

        let _ = self.pool.set(pool);
        Ok(())
    }

    async fn init(&self) -> BuckyResult<()> {
        Ok(())
    }

    async fn get_desc(&self, obj_type: u8) -> BuckyResult<u64> {
        let row = sqlx::query(GET_OBJ_DESC_NUM).bind(obj_type).fetch_one(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        let sum: i64 = row.try_get(0).unwrap_or(0);
        Ok(sum as u64)
    }

    async fn get_desc_add(&self, obj_type: u8, period: Period) -> BuckyResult<u64> {
        let now = bucky_time_now();
        let mut start = bucky_time_to_js_time(now);
        if period == Period::Daily {
            start -= 86400 * 1000;
        } else if period == Period::Weekly {
            start -= 7 * 86400 * 1000;
        } else {
            start -= 30 * 86400 * 1000;
        }
        let start = js_time_to_bucky_time(start);

        let row = sqlx::query(GET_OBJ_ADD_DESC_NUM).bind(obj_type).bind(start as i64).bind(now as i64).fetch_one(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        let sum: i64 = row.try_get(0).unwrap_or(0);
        Ok(sum as u64)
    }

    async fn get_desc_active(&self, obj_type: u8, period: Period) -> BuckyResult<u64> {
        let now = bucky_time_now();
        let mut start = bucky_time_to_js_time(now);
        if period == Period::Daily {
            start -= 86400 * 1000;
        } else if period == Period::Weekly {
            start -= 7 * 86400 * 1000;
        } else {
            start -= 30 * 86400 * 1000;
        }
        let start = js_time_to_bucky_time(start);

        let row = sqlx::query(GET_OBJ_ACTIVE_DESC_NUM).bind(obj_type).bind(start as i64).bind(now as i64).fetch_one(self.pool.get().unwrap()).await.map_err(map_sql_err)?;
        let sum: i64 = row.try_get(0).unwrap_or(0);
        Ok(sum as u64)
    }

    async fn get_meta_stat(&self, meta_type: u8, period: Period) -> BuckyResult<Vec<MetaStat>> {
        let rows = if 0 == meta_type {
            let now = bucky_time_now();
            let mut start = bucky_time_to_js_time(now);
            if period == Period::Daily {
                start -= 86400 * 1000;
            } else if period == Period::Weekly {
                start -= 7 * 86400 * 1000;
            } else {
                start -= 30 * 86400 * 1000;
            }
            let start = js_time_to_bucky_time(start);
            sqlx::query(GET_META_OBJ_NUM)
                .bind(start as i64)
                .bind(now as i64)
        } else {
            sqlx::query(GET_META_API_NUM)
        }
        .fetch_all(self.pool.get().unwrap()).await.map_err(map_sql_err)?;

        let mut ret = Vec::new();
        for row in rows {
            ret.push(self.metainfo_from_row(&row));
        }
        Ok(ret)
    }
}