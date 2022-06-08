use std::marker::PhantomData;
use std::str::FromStr;
use std::time::Duration;
use cyfs_base::{BuckyError, BuckyErrorCode};
use sqlx::{Transaction, Connection, Executor, ConnectOptions};
use log::LevelFilter;
use sqlx::pool::PoolConnection;
use sqlx::Execute;
pub use sqlx as cyfs_sql;
pub use sqlx::Row as SqlRow;

pub trait ErrorMap {
    type OutError;
    type InError;
    fn map(e: Self::InError, msg: &str) -> Self::OutError;
}

pub struct DefaultToBuckyError;

impl ErrorMap for DefaultToBuckyError {
    type OutError = BuckyError;
    type InError = sqlx::Error;

    fn map(e: sqlx::Error, msg: &str) -> BuckyError {
        match e {
            sqlx::Error::RowNotFound => {
                let msg = format!("not found, {}", msg);
                BuckyError::new(BuckyErrorCode::NotFound, msg)
            }
            _ => {
                let msg = format!("sqlite error: {:?} info:{}", e, msg);
                if cfg!(test) {
                    println!("{}", msg);
                } else {
                    log::error!("{}", msg);
                }
                BuckyError::new(BuckyErrorCode::SqliteError, msg)
            }
        }
    }
}

pub type SqlResult = <sqlx::Any as sqlx::Database>::QueryResult;
pub type SqlRowObject = <sqlx::Any as sqlx::Database>::Row;
pub type SqlTransaction<'a> = sqlx::Transaction<'a, sqlx::Any>;
pub type SqlQuery<'a> = sqlx::query::Query<'a, sqlx::Any, <sqlx::Any as sqlx::database::HasArguments<'a>>::Arguments>;
pub type RawSqlPool = sqlx::AnyPool;

#[macro_export]
macro_rules! sql_query {
    ($query:expr) => ({
        cyfs_sql::query!($query)
    });

    ($query:expr, $($args:tt)*) => ({
        cyfs_sql::query!($query, $($args)*)
    })
}

#[derive(Clone)]
pub struct SqlPool<EM: ErrorMap<InError = sqlx::Error> = DefaultToBuckyError> {
    pool: sqlx::AnyPool,
    uri: String,
    _em: PhantomData<EM>,
}

impl<EM: ErrorMap<InError = sqlx::Error>> SqlPool<EM> {
    pub fn from_raw_pool(pool: RawSqlPool) -> Self {
        Self { pool, uri: "".to_string(), _em: Default::default() }
    }

    pub async fn open(uri: &str, max_connections: u32) -> Result<Self, EM::OutError> {
        log::info!("open pool {} max_connections {}", uri, max_connections);
        let pool_options = sqlx::any::AnyPoolOptions::new()
            .max_connections(max_connections)
            .connect_timeout(Duration::from_secs(300))
            .min_connections(0)
            .idle_timeout(Duration::from_secs(300));
        let kind = sqlx::any::AnyKind::from_str(uri).map_err(|e| {
            EM::map(e, format!("[{} {}]", line!(), uri).as_str())
        })?;
        let pool = match kind {
            sqlx::any::AnyKind::Sqlite => {
                let mut options = sqlx::sqlite::SqliteConnectOptions::from_str(uri).map_err(|e| {
                    EM::map(e, format!("[{} {}]", line!(), uri).as_str())
                })?
                    .busy_timeout(Duration::from_secs(300))
                    .create_if_missing(true);
                #[cfg(target_os = "ios")]
                {
                    options = options.serialized(true);
                }

                options.log_statements(LevelFilter::Off)
                    .log_slow_statements(LevelFilter::Off, Duration::from_secs(10));
                pool_options.connect_with(sqlx::any::AnyConnectOptions::from(options)).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), uri).as_str()))?
            },
            _ => {
                pool_options.connect(uri).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), uri).as_str()))?
            }
        };
        Ok(Self {
            pool,
            uri: uri.to_string(),
            _em: Default::default()
        })
    }

    pub async fn raw_pool(&self) -> RawSqlPool {
        self.pool.clone()
    }

    pub async fn get_conn(&self) -> Result<SqlConnection, EM::OutError> {
        let conn = self.pool.acquire().await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), self.uri.as_str()).as_str()))?;
        Ok(SqlConnection::from(conn))
    }
}

pub fn sql_query(sql: &str) -> SqlQuery<'_> {
    sqlx::query::<sqlx::Any>(sql)
}

pub enum SqlConnectionType {
    PoolConn(PoolConnection<sqlx::Any>),
    Conn(sqlx::AnyConnection),
}
pub struct SqlConnection<EM: ErrorMap<InError = sqlx::Error> = DefaultToBuckyError> {
    conn: SqlConnectionType,
    trans: Option<Transaction<'static, sqlx::Any>>,
    _em: PhantomData<EM>,
}

impl From<sqlx::pool::PoolConnection<sqlx::Any>> for SqlConnection {
    fn from(conn: sqlx::pool::PoolConnection<sqlx::Any>) -> Self {
        Self { conn: SqlConnectionType::PoolConn(conn), _em: Default::default(), trans: None }
    }
}

impl<EM: 'static + ErrorMap<InError = sqlx::Error>> SqlConnection<EM> {
    pub async fn open(uri: &str) -> Result<Self, EM::OutError> {
        let kind = sqlx::any::AnyKind::from_str(uri).map_err(|e| EM::map(e, format!("[{} {}]", line!(), uri).as_str()))?;
        let conn = match kind {
            sqlx::any::AnyKind::Sqlite => {
                let mut options = sqlx::sqlite::SqliteConnectOptions::from_str(uri).map_err(|e| EM::map(e, format!("[{} {}]", line!(), uri).as_str()))?
                    .busy_timeout(Duration::from_secs(300));
                #[cfg(target_os = "ios")]
                {
                    options = options.serialized(true);
                }

                options.log_statements(LevelFilter::Off)
                    .log_slow_statements(LevelFilter::Off, Duration::from_secs(10));
                sqlx::any::AnyConnectOptions::from(options).connect().await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), uri).as_str()))?
            },
            _ => {
                sqlx::any::AnyConnection::connect(uri).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), uri).as_str()))?
            }
        };

        Ok(Self {
            conn: SqlConnectionType::Conn(conn),
            _em: Default::default(),
            trans: None
        })
    }

    pub async fn execute_sql(&mut self, query: SqlQuery<'_>) -> Result<SqlResult, EM::OutError> {
        let sql = query.sql();
        log::debug!("sql {}", sql);
        if self.trans.is_none() {
            match &mut self.conn {
                SqlConnectionType::PoolConn(conn) => {
                    conn.execute(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
                },
                SqlConnectionType::Conn(conn) => {
                    conn.execute(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
                }
            }
        } else {
            self.trans.as_mut().unwrap().execute(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
        }
    }

    pub async fn query_one(&mut self, query: SqlQuery<'_>) -> Result<SqlRowObject, EM::OutError> {
        let sql = query.sql();
        log::debug!("sql {}", sql);
        if self.trans.is_none() {
            match &mut self.conn {
                SqlConnectionType::PoolConn(conn) => {
                    conn.fetch_one(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
                },
                SqlConnectionType::Conn(conn) => {
                    conn.fetch_one(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
                }
            }
        } else {
            self.trans.as_mut().unwrap().fetch_one(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
        }
    }

    pub async fn query_all(&mut self, query: SqlQuery<'_>) -> Result<Vec<SqlRowObject>, EM::OutError> {
        let sql = query.sql();
        log::debug!("sql {}", sql);
        if self.trans.is_none() {
            match &mut self.conn {
                SqlConnectionType::PoolConn(conn) => {
                    conn.fetch_all(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
                },
                SqlConnectionType::Conn(conn) => {
                    conn.fetch_all(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
                }
            }
        } else {
            self.trans.as_mut().unwrap().fetch_all(query).await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), sql).as_str()))
        }
    }

    pub async fn begin_transaction(&mut self) -> Result<(), EM::OutError> {
        let this: &'static mut Self = unsafe {std::mem::transmute(self)};
        let trans = match &mut this.conn {
            SqlConnectionType::PoolConn(conn) => {
                conn.begin().await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), "begin trans").as_str()))
            },
            SqlConnectionType::Conn(conn) => {
                conn.begin().await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), "begin trans").as_str()))
            }
        }?;
        this.trans = Some(trans);
        Ok(())
    }

    pub async fn rollback_transaction(&mut self) -> Result<(), EM::OutError> {
        if self.trans.is_none() {
            return Ok(())
        } else {
            self.trans.take().unwrap().rollback().await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), "rollback trans").as_str()))
        }
    }

    pub async fn commit_transaction(&mut self) -> Result<(), EM::OutError> {
        if self.trans.is_none() {
            return Ok(())
        } else {
            self.trans.take().unwrap().commit().await.map_err(|e| EM::map(e, format!("[{} {}]", line!(), "commit trans").as_str()))
        }
    }
}

impl<EM: ErrorMap<InError=sqlx::Error>> Drop for SqlConnection<EM> {
    fn drop(&mut self) {
        if self.trans.is_some() {
            let trans = self.trans.take().unwrap();
            async_std::task::block_on(async move {
                let _ = trans.rollback().await;
            });
        }
    }
}

#[cfg(test)]
mod test_connection {
    use cyfs_base::BuckyResult;
    use sqlx::{Connection, Row};
    use crate::*;

    #[derive(sqlx::FromRow)]
    struct DescExtra {
        obj_id: String,
        rent_arrears: i64,
        rent_arrears_count: i64,
        rent_value: i64,
        coin_id: i8,
        data_len: i32,
        other_charge_balance: i64,
    }

    async fn new<DB>() -> BuckyResult<DB::Connection>
        where
            DB: sqlx::Database,
    {
        Ok(DB::Connection::connect("sqlite::memory:").await.map_err(|e|DefaultToBuckyError::map(e, ""))?)
    }

    #[test]
    fn test() {
        async_std::task::block_on(async {
            let mut sqlx_conn = SqlConnection::<DefaultToBuckyError>::open("sqlite://:memory:").await.unwrap();
            sqlx_conn.begin_transaction().await.unwrap();
            let create_table = r#"CREATE TABLE IF NOT EXISTS desc_extra (
            "obj_id" char(45) PRIMARY KEY NOT NULL UNIQUE,
        "rent_arrears" INTEGER,
        "rent_arrears_count" INTEGER,
        "rent_value" INTEGER,
        "coin_id" INTEGER,
        "data_len" INTEGER,
        "other_charge_balance" INTEGER);"#;
            sqlx_conn.execute_sql(sql_query(create_table)).await.unwrap();
            let insert = r#"insert into desc_extra (obj_id,
            rent_arrears,
            rent_arrears_count,
            rent_value,
            coin_id,
            data_len,
            other_charge_balance) values (
            "test", 1, 1, 2, 3, 4, 5)"#;
            sqlx_conn.execute_sql(sql_query(insert)).await.unwrap();
            sqlx_conn.commit_transaction().await.unwrap();

            let query = sql_query("select * from desc_extra where obj_id = ?").bind("test");
            let row = sqlx_conn.query_one(query).await.unwrap();
            let id: String = row.get("obj_id");
            assert_eq!(id, "test".to_owned());
            let coin_id: i32 = row.get("coin_id");
            assert_eq!(coin_id, 3);

            let row = sqlx_conn.query_one(sqlx::query("select * from desc_extra where obj_id = ?").bind("test")).await.unwrap();
            let id: String = row.get("obj_id");
            assert_eq!(id, "test".to_owned());
            let coin_id: i32 = row.get("coin_id");
            assert_eq!(coin_id, 3);
            //
            // let query = sqlx::query_as::<_, DescExtra>("select * from desc_extra where obj_id = ?").bind("test").query_one(&mut sqlx_conn).await.unwrap();
            // assert_eq!(query.obj_id, "test".to_owned());
            // assert_eq!(query.coin_id, 3);
        })
    }

    #[test]
    fn test_pool() {
        async_std::task::block_on(async {
            let pool = SqlPool::<DefaultToBuckyError>::open("sqlite://:memory:", 5).await.unwrap();

            let mut sqlx_conn = pool.get_conn().await.unwrap();
            let create_table = r#"CREATE TABLE IF NOT EXISTS desc_extra (
            "obj_id" char(45) PRIMARY KEY NOT NULL UNIQUE,
        "rent_arrears" INTEGER,
        "rent_arrears_count" INTEGER,
        "rent_value" INTEGER,
        "coin_id" INTEGER,
        "data_len" INTEGER,
        "other_charge_balance" INTEGER);"#;
            sqlx_conn.execute_sql(sql_query(create_table)).await.unwrap();

            sqlx_conn.begin_transaction().await.unwrap();
            let insert = r#"insert into desc_extra (obj_id,
            rent_arrears,
            rent_arrears_count,
            rent_value,
            coin_id,
            data_len,
            other_charge_balance) values (
            "test", 1, 1, 2, 3, 4, 5)"#;
            sqlx_conn.execute_sql(sql_query(insert)).await.unwrap();
            sqlx_conn.rollback_transaction().await.unwrap();

            let mut sqlx_conn = pool.get_conn().await.unwrap();
            let query = sqlx::query("select * from desc_extra where obj_id = ?").bind("test");
            let row = sqlx_conn.query_all(query).await.unwrap();
            assert_eq!(row.len(), 0);

            let mut sqlx_conn = pool.get_conn().await.unwrap();
            sqlx_conn.begin_transaction().await.unwrap();
            let insert = r#"insert into desc_extra (obj_id,
            rent_arrears,
            rent_arrears_count,
            rent_value,
            coin_id,
            data_len,
            other_charge_balance) values (
            "test", 1, 1, 2, 3, 4, 5)"#;
            sqlx_conn.execute_sql(sql_query(insert)).await.unwrap();
            sqlx_conn.commit_transaction().await.unwrap();

            let query = sqlx::query("select * from desc_extra where obj_id = ?").bind("test");
            let row = sqlx_conn.query_one(query).await.unwrap();
            let id: String = row.get("obj_id");
            assert_eq!(id, "test".to_owned());
            let coin_id: i32 = row.get("coin_id");
            assert_eq!(coin_id, 3);

            let row = sqlx_conn.query_one(sqlx::query("select * from desc_extra where obj_id = ?").bind("test")).await.unwrap();
            let id: String = row.get("obj_id");
            assert_eq!(id, "test".to_owned());
            let coin_id: i32 = row.get("coin_id");
            assert_eq!(coin_id, 3);
        })
    }
}
