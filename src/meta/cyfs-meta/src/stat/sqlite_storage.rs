use std::str::FromStr;
use std::time::Duration;
use async_trait::async_trait;
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{ConnectOptions, SqlitePool};
use cyfs_base::BuckyResult;
use crate::DBExecutor;
use crate::stat::{StatCache, Storage};

#[derive(Serialize, Deserialize)]
pub struct SqliteConfig {
    path: String
}

pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    pub fn new(config: SqliteConfig) -> Self {
        let mut options = SqliteConnectOptions::from_str(&format!("sqlite://{}", &config.path)).unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory);
        options
            .log_statements(LevelFilter::Off)
            .log_slow_statements(LevelFilter::Off, Duration::new(10, 0));

        Self {
            pool: sqlx::Pool::connect_lazy_with(options),
        }
    }
}

const CREATE_DESC_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "create_desc" (
	"objectid"	VARCHAR(45) NOT NULL,
	"object_type"	INTEGER NOT NULL,
	"create_time"	DATETIME NOT NULL,
	PRIMARY KEY("objectid")
)
"#;

const API_CALL_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "api_call" (
	"name"	TEXT NOT NULL,
	"ret"	INTEGER NOT NULL,
	"time"	DATETIME NOT NULL
)
"#;

const QUERY_DESC_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "query_desc" (
	"objectid"	VARCHAR(45) NOT NULL,
	"exists"	BOOL NOT NULL,
	"time"	DATETIME NOT NULL
)
"#;

const INSERT_CREATE_DESC: &str = r#"INSERT INTO "create_desc" VALUES (?1,?2,?3)"#;
const INSERT_API_CALL: &str = r#"INSERT INTO "api_call" VALUES (?1,?2,?3)"#;
const INSERT_QUERY_DESC: &str = r#"INSERT INTO "query_desc" VALUES (?1,?2,?3)"#;

#[async_trait]
impl Storage for SqliteStorage {
    async fn init(&self) -> BuckyResult<()> {
        let mut conn = self.pool.acquire().await?;
        conn.execute_sql(CREATE_DESC_TABLE).await?;
        conn.execute_sql(API_CALL_TABLE).await?;
        conn.execute_sql(QUERY_DESC_TABLE).await?;
        
        Ok(())
    }

    async fn save(&self, cache: StatCache) -> BuckyResult<()> {
        for (id, time) in &cache.add_desc_stat {
            sqlx::query(INSERT_CREATE_DESC).bind(id.to_string()).bind(id.obj_type_code() as u8).bind(time).execute(&self.pool).await?;
        }

        for (name, ret, time) in &cache.api_call {
            sqlx::query(INSERT_API_CALL).bind(name).bind(ret).bind(time).execute(&self.pool).await?;
        }

        for (id, exists, time) in &cache.query_desc {
            sqlx::query(INSERT_QUERY_DESC).bind(id.to_string()).bind(exists).bind(time).execute(&self.pool).await?;
        }

        Ok(())
    }
}