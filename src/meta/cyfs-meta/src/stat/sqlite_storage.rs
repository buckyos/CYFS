use std::str::FromStr;
use std::time::Duration;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{StreamExt};
use log::{error, info, LevelFilter};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{ConnectOptions, Executor, Row, SqlitePool};
use cyfs_base::{Area, BuckyError, BuckyResult, ObjectId, ObjectIdInfo, ObjectTypeCode};
use crate::{ArcWeakHelper, DBExecutor, StateWeakRef};
use crate::stat::{MemoryStat, StatCache, Storage};

#[derive(Serialize, Deserialize)]
pub struct SqliteConfig {
    path: String
}

pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    pub fn new(config: SqliteConfig, read_only: bool) -> Self {
        let mut options = SqliteConnectOptions::from_str(&format!("sqlite://{}", &config.path)).unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory).read_only(read_only);
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

const DESC_TABLE: &str = r#"
CREATE TABLE "desc" (
	"objectid"	VARCHAR(45) NOT NULL,
	"type_code"	INTEGER NOT NULL,
	"area_country"	INTEGER NOT NULL,
	"area_carrier"	INTEGER NOT NULL,
	"area_city"	INTEGER NOT NULL,
	PRIMARY KEY("objectid")
);
"#;

const INSERT_CREATE_DESC: &str = r#"INSERT INTO "create_desc" VALUES (?1,?2,?3)"#;
const INSERT_API_CALL: &str = r#"INSERT INTO "api_call" VALUES (?1,?2,?3)"#;
const INSERT_QUERY_DESC: &str = r#"INSERT INTO "query_desc" VALUES (?1,?2,?3)"#;
const INSERT_DESC: &str = r#"INSERT INTO "desc" VALUES (?1,?2,?3,?4,?5)"#;

const QUERY_CREATE_DESC: &str = r#"select count(objectid) as num from "create_desc" where object_type = ?1 and create_time > ?2"#;
const QUERY_QUERY_DESC: &str = r#"select objectid from "query_desc" where "exists" = 1 and time > ?1"#;
const QUERY_API_SUCCESS: &str = r#"select name, count(name) as success from "api_call" where ret = 0 and time > ?1 group by name"#;
const QUERY_API_FAILED: &str = r#"select name, count(name) as failed from "api_call" where ret > 0 and time > ?1 group by name"#;
const QUERY_DESC: &str = r#"SELECT count(*) as num from desc"#;

fn object_id_to_info(id: &ObjectId) -> (Area, u8) {
    match id.info() {
        ObjectIdInfo::Data(_) => {
            (Area::default(), 19)
        }
        ObjectIdInfo::Standard(info) => {
            (info.area.unwrap_or(Area::default()), info.obj_type_code as u8)
        }
        ObjectIdInfo::Core(info) => {
            (info.area.unwrap_or(Area::default()), 17)
        }
        ObjectIdInfo::DecApp(info) => {
            (info.area.unwrap_or(Area::default()), 18)
        }
    }
}

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
            let (area, type_code) = object_id_to_info(&id);
            let _ = sqlx::query(INSERT_DESC).bind(id.to_string())
                .bind(type_code)
                .bind(area.country)
                .bind(area.carrier)
                .bind(area.city)
                .execute(&self.pool).await;
        }

        for (name, ret, time) in &cache.api_call {
            sqlx::query(INSERT_API_CALL).bind(name).bind(ret).bind(time).execute(&self.pool).await?;
        }

        for (id, exists, time) in &cache.query_desc {
            sqlx::query(INSERT_QUERY_DESC).bind(id.to_string()).bind(exists).bind(time).execute(&self.pool).await?;
        }

        Ok(())
    }

    async fn get_stat(&self, from: DateTime<Utc>) -> BuckyResult<MemoryStat> {
        let mut stat = MemoryStat::default();
        stat.new_people = sqlx::query(QUERY_CREATE_DESC).bind(ObjectTypeCode::People as u8).bind(from).fetch_one(&self.pool).await?.try_get("num")?;
        stat.new_device = sqlx::query(QUERY_CREATE_DESC).bind(ObjectTypeCode::Device as u8).bind(from).fetch_one(&self.pool).await?.try_get("num")?;

        let rets = sqlx::query(QUERY_QUERY_DESC).bind(from).fetch_all(&self.pool).await?;
        for ret in rets {
            let object_id: String = ret.try_get("objectid")?;
            let objid = ObjectId::from_str(&object_id)?;
            match objid.obj_type_code() {
                ObjectTypeCode::People => {
                    stat.active_people.insert(objid);
                },
                ObjectTypeCode::Device => {
                    stat.active_device.insert(objid);
                },
                _ => {}
            }
        }

        let success_apis = sqlx::query(QUERY_API_SUCCESS).bind(from).fetch_all(&self.pool).await?;
        for success_api in success_apis {
            let name: String = success_api.try_get("name")?;
            let num: u32 = success_api.try_get("success")?;
            stat.api_success.insert(name, num);
        }

        let failed_apis = sqlx::query(QUERY_API_FAILED).bind(from).fetch_all(&self.pool).await?;
        for failed_api in failed_apis {
            let name: String = failed_api.try_get("name")?;
            let num: u32 = failed_api.try_get("failed")?;
            stat.api_fail.insert(name, num);
        }
        Ok(stat)

    }

    async fn is_stat_desc(&self) -> BuckyResult<bool> {
        let ret: i32 = sqlx::query("SELECT count(*) as ret FROM sqlite_master WHERE type=\"table\" AND name = \"desc\"").fetch_one(&self.pool).await?.try_get(0)?;
        info!("count desc table ret {}", ret);
        Ok(ret == 1)
    }

    async fn stat_desc(&self, state: StateWeakRef) -> BuckyResult<()> {
        sqlx::query(DESC_TABLE).execute(&self.pool).await?;
        let rc_state = state.to_rc().unwrap();
        let mut conn = rc_state.get_conn().await;
        let mut stream = conn.fetch(sqlx::query("select obj_id from all_descs")).map(|row|{
            match row {
                Ok(row) => {
                    let id_str: String = row.try_get(0)?;
                    let id = ObjectId::from_str(&id_str)?;
                    Ok(id)
                }
                Err(e) => Err(BuckyError::from(e))
            }
        });
        info!("fetching state desc table");
        while let Some(id) = stream.next().await {
            match id {
                Ok(id) => {
                    let (area, type_code) = object_id_to_info(&id);
                    if let Err(e) = sqlx::query(INSERT_DESC).bind(id.to_string())
                        .bind(type_code)
                        .bind(area.country)
                        .bind(area.carrier)
                        .bind(area.city)
                        .execute(&self.pool).await {
                        error!("store stat desc table err {}", e);
                        sqlx::query("DROP TABLE desc").execute(&self.pool).await?;
                        return Err(BuckyError::from(e));
                    }
                }
                Err(e) => {
                    error!("fetch state desc table err {}", e);
                    sqlx::query("DROP TABLE desc").execute(&self.pool).await?;
                    return Err(BuckyError::from(e));
                }
            }
        }

        Ok(())
    }

    async fn get_desc_total(&self, obj_type: Option<ObjectTypeCode>) -> BuckyResult<u64> {
        let mut sql = QUERY_DESC.to_owned();
        let mut first_where = true;
        if obj_type.is_some() {
            if first_where {
                sql = sql + " where ";
                first_where = false;
            }
            sql = sql + "type_code = ?1";
        }

        let mut query = sqlx::query(&sql);
        if let Some(obj_type) = obj_type {
            query = query.bind(obj_type as u8);
        }
        let num: i64 = query.fetch_one(&self.pool).await?.try_get(0)?;
        Ok(num as u64)
    }
}