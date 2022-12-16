use cyfs_base::{BuckyError, BuckyResult};
use sqlx::{Transaction, Connection, Executor, Database, IntoArguments, FromRow, Sqlite};
use async_trait::async_trait;
use sqlx::query::{Query, QueryAs};
use crate::*;

pub fn map_sql_err(e: sqlx::Error) -> BuckyError {
    match e {
        sqlx::Error::RowNotFound => {
            crate::meta_err!(ERROR_NOT_FOUND)
        }
        _ => {
            let msg = format!("sqlite error: {:?}", e);
            if cfg!(test) {
                println!("{}", msg);
            } else {
                log::error!("{}", msg);
            }
            crate::meta_err2!(ERROR_EXCEPTION, msg)
        }
    }
}

#[async_trait]
pub trait DBExecutor<DB: sqlx::Database> {
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<DB::QueryResult>;
    async fn query_one<'e, E: 'e + sqlx::Execute<'e, DB>>(&mut self, query: E) -> BuckyResult<DB::Row>;
    async fn query_all<'e, E: 'e + sqlx::Execute<'e, DB>>(&mut self, query: E) -> BuckyResult<Vec<DB::Row>>;
    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool>;
    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()>;
}

#[async_trait]
pub trait DBQuery<'a, DB: sqlx::Database> {
    async fn execute_sql<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<DB::QueryResult>
        where
            E: Executor<'c, Database = DB>,;
    async fn query_one<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<DB::Row>
        where
            E: Executor<'c, Database = DB>;
    async fn query_all<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<Vec<DB::Row>>
        where
            E: Executor<'c, Database = DB>;
}

#[async_trait]
impl <'a, DB: sqlx::Database, A> DBQuery<'a, DB> for Query<'a, DB, A>
where
    A: 'a + IntoArguments<'a, DB>
{
    async fn execute_sql<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<<DB as Database>::QueryResult> where
        E: Executor<'c, Database=DB>, {
        self.execute(executor).await.map_err(map_sql_err)
    }

    async fn query_one<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<DB::Row>
        where
            E: Executor<'c, Database = DB>,
    {
        self.fetch_one(executor).await.map_err(map_sql_err)
    }

    async fn query_all<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<Vec<DB::Row>>
        where
            E: Executor<'c, Database = DB>,
    {
        self.fetch_all(executor).await.map_err(map_sql_err)
    }
}

#[async_trait]
pub trait DBQueryAs<'a, DB: sqlx::Database, O, A>
    where
        A: 'a + IntoArguments<'a, DB>,
        O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    async fn query_one<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<O>
    where
        'a: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: 'e,
        A: 'e;
    async fn query_all<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<Vec<O>>
        where
            'a: 'e,
            E: 'e + Executor<'c, Database = DB>,
            DB: 'e,
            O: 'e,
            A: 'e;
}

#[async_trait]
impl <'a, DB: sqlx::Database, O, A> DBQueryAs<'a, DB, O, A> for QueryAs<'a, DB, O, A>
where
    A: 'a + IntoArguments<'a, DB>,
    O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    async fn query_one<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<O> where
        'a: 'e,
        E: 'e + Executor<'c, Database=DB>,
        DB: 'e,
        O: 'e,
        A: 'e {
        self.fetch_one(executor).await.map_err(map_sql_err)
    }

    async fn query_all<'e, 'c: 'e, E>(self, executor: E) -> BuckyResult<Vec<O>> where
        'a: 'e,
        E: 'e + Executor<'c, Database=DB>,
        DB: 'e,
        O: 'e,
        A: 'e {
        self.fetch_all(executor).await.map_err(map_sql_err)
    }
}

pub trait DBTransactionSqlCreator {
    fn begin_transaction_sql(pos: Option<String>) -> String;
    fn commit_transaction_sql(pos: Option<String>) -> String;
    fn rollback_transaction_sql(pos: Option<String>) -> String;
}

#[async_trait]
pub trait  DBConnection<DB: sqlx::Database>
{
    async fn begin_transaction(&mut self) -> BuckyResult<Transaction<'_, DB>>;
}

#[async_trait]
pub trait DBTransaction {
    async fn rollback_transaction(mut self) -> BuckyResult<()>;
    async fn commit_transaction(mut self) -> BuckyResult<()>;
}

pub struct AnsiDBTransactionSqlCreator {}

impl DBTransactionSqlCreator for AnsiDBTransactionSqlCreator {
    fn begin_transaction_sql(pos: Option<String>) -> String {
        if pos.is_none() {
            "BEGIN".to_owned()
        } else {
            format!("SAVEPOINT _db_savepoint_{}", pos.unwrap())
        }
    }

    fn commit_transaction_sql(pos: Option<String>) -> String {
        if pos.is_none() {
            "COMMIT".to_owned()
        } else {
            format!("RELEASE SAVEPOINT _db_savepoint_{}", pos.unwrap())
        }
    }

    fn rollback_transaction_sql(pos: Option<String>) -> String {
        if pos.is_none() {
            "ROLLBACK".to_owned()
        } else {
            format!("ROLLBACK TO _db_savepoint_{}", pos.unwrap())
        }
    }
}

#[async_trait]
impl DBConnection<sqlx::Sqlite> for sqlx::SqliteConnection {
    async fn begin_transaction(&mut self) -> BuckyResult<Transaction<'_, sqlx::Sqlite>> {
        let tx = self.begin().await.map_err(map_sql_err)?;
        Ok(tx)
    }

}

#[async_trait]
#[cfg(any(not(db_type), db_type = "sqlite"))]
impl<'a> DBExecutor<sqlx::Sqlite> for sqlx::SqliteConnection {
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<<sqlx::Sqlite as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<<sqlx::Sqlite as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::Sqlite as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool> {
        let sql = r#"select * from sqlite_master where name=?1 and sql like ?2"#;
        let ret = self.fetch_one(sqlx::query(sql)
            .bind(table_name).bind(format!("%{}%", column_name))).await;
        if let Err(_) = &ret {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()> {
        let sql = format!(r#"alter table {} add column {}"#, table_name, column);
        self.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }
}

#[async_trait]
#[cfg(any(not(db_type), db_type = "sqlite"))]
impl<'a> DBExecutor<sqlx::Sqlite> for sqlx::pool::PoolConnection<Sqlite> {
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<<sqlx::Sqlite as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<<sqlx::Sqlite as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::Sqlite as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool> {
        let sql = r#"select * from sqlite_master where name=?1 and sql like ?2"#;
        let ret = self.fetch_one(sqlx::query(sql)
            .bind(table_name).bind(format!("%{}%", column_name))).await;
        if let Err(_) = &ret {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()> {
        let sql = format!(r#"alter table {} add column {}"#, table_name, column);
        self.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }
}

// #[async_trait]
// impl <'a, DB: sqlx::Database, T: sqlx::Executor<'a>> DBExecutor<DB> for T {
//     async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<<DB as Database>::QueryResult> {
//         self.execute(query).await.map_err(map_sql_err)
//     }
//
//     async fn query_one<'e, E: 'e + sqlx::Execute<'e, DB>>(&mut self, query: E) -> BuckyResult<<DB as Database>::Row> {
//         self.fetch_one(query).await.map_err(map_sql_err)
//     }
//
//     async fn query_all<'e, E: 'e + sqlx::Execute<'e, DB>>(&mut self, query: E) -> BuckyResult<Vec<<DB as Database>::Row>> {
//         self.fetch_all(query).await.map_err(map_sql_err)
//     }
// }

#[async_trait]
#[cfg(db_type = "mssql")]
impl<'a> DBExecutor<sqlx::Mssql> for sqlx::MssqlConnection {
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Mssql>>(&mut self, query: E) -> BuckyResult<<sqlx::Mssql as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::Mssql>>(&mut self, query: E) -> BuckyResult<<sqlx::Mssql as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::Mssql>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::Mssql as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool> {
        unimplemented!()
    }

    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()> {
        unimplemented!()
    }
}

#[async_trait]
#[cfg(db_type = "mysql")]
impl<'a> DBExecutor<sqlx::MySql> for sqlx::MySqlConnection {
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::MySql>>(&mut self, query: E) -> BuckyResult<<sqlx::MySql as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::MySql>>(&mut self, query: E) -> BuckyResult<<sqlx::MySql as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::MySql>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::MySql as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool> {
        unimplemented!()
    }

    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()> {
        unimplemented!()
    }
}

#[async_trait]
#[cfg(db_type = "postgres")]
impl<'a> DBExecutor<sqlx::Postgres> for sqlx::PgConnection {
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Postgres>>(&mut self, query: E) -> BuckyResult<<sqlx::Postgres as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::Postgres>>(&mut self, query: E) -> BuckyResult<<sqlx::Postgres as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::Postgres>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::Postgres as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool> {
        unimplemented!()
    }

    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()> {
        unimplemented!()
    }
}

#[async_trait]
#[cfg(any(not(db_type), db_type = "sqlite"))]
impl<'a> DBExecutor<sqlx::Sqlite> for sqlx::Transaction<'a, sqlx::Sqlite>
{
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<<sqlx::Sqlite as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<<sqlx::Sqlite as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::Sqlite>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::Sqlite as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, _table_name: &str, _column_name: &str) -> BuckyResult<bool> {
        unimplemented!()
    }

    async fn add_column(&mut self, _table_name: &str, _column: &str) -> BuckyResult<()> {
        unimplemented!()
    }
}

#[async_trait]
#[cfg(db_type = "mssql")]
impl<'a> DBExecutor<sqlx::Mssql> for sqlx::Transaction<'a, sqlx::Mssql>
{
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Mssql>>(&mut self, query: E) -> BuckyResult<<sqlx::Mssql as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::Mssql>>(&mut self, query: E) -> BuckyResult<<sqlx::Mssql as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::Mssql>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::Mssql as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool> {
        unimplemented!()
    }

    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()> {
        unimplemented!()
    }
}

#[async_trait]
#[cfg(db_type = "mysql")]
impl<'a> DBExecutor<sqlx::MySql> for sqlx::Transaction<'a, sqlx::MySql>
{
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::MySql>>(&mut self, query: E) -> BuckyResult<<sqlx::MySql as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::MySql>>(&mut self, query: E) -> BuckyResult<<sqlx::MySql as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::MySql>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::MySql as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool> {
        unimplemented!()
    }

    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()> {
        unimplemented!()
    }
}

#[async_trait]
#[cfg(db_type = "postgres")]
impl<'a> DBExecutor<sqlx::Postgres> for sqlx::Transaction<'a, sqlx::Postgres>
{
    async fn execute_sql<'e, E: 'e + sqlx::Execute<'e, sqlx::Postgres>>(&mut self, query: E) -> BuckyResult<<sqlx::Postgres as Database>::QueryResult> {
        self.execute(query).await.map_err(map_sql_err)
    }

    async fn query_one<'e, E: 'e + sqlx::Execute<'e, sqlx::Postgres>>(&mut self, query: E) -> BuckyResult<<sqlx::Postgres as Database>::Row> {
        self.fetch_one(query).await.map_err(map_sql_err)
    }

    async fn query_all<'e, E: 'e + sqlx::Execute<'e, sqlx::Postgres>>(&mut self, query: E) -> BuckyResult<Vec<<sqlx::Postgres as Database>::Row>> {
        self.fetch_all(query).await.map_err(map_sql_err)
    }

    async fn has_column(&mut self, table_name: &str, column_name: &str) -> BuckyResult<bool> {
        unimplemented!()
    }

    async fn add_column(&mut self, table_name: &str, column: &str) -> BuckyResult<()> {
        unimplemented!()
    }
}

#[async_trait]
#[cfg(any(not(db_type), db_type = "sqlite"))]
impl<'a> DBTransaction for sqlx::Transaction<'a, sqlx::Sqlite>
{
    async fn rollback_transaction(mut self) -> BuckyResult<()> {
        self.rollback().await.map_err(map_sql_err)
    }

    async fn commit_transaction(mut self) -> BuckyResult<()> {
        self.commit().await.map_err(map_sql_err)
    }
}

#[async_trait]
#[cfg(db_type = "mssql")]
impl<'a> DBTransaction for sqlx::Transaction<'a, sqlx::MsSql>
{
    async fn rollback_transaction(mut self) -> BuckyResult<()> {
        self.rollback().await.map_err(map_sql_err)
    }

    async fn commit_transaction(mut self) -> BuckyResult<()> {
        self.commit().await.map_err(map_sql_err)
    }
}

#[async_trait]
#[cfg(db_type = "mysql")]
impl<'a> DBTransaction for sqlx::Transaction<'a, sqlx::MySql>
{
    async fn rollback_transaction(mut self) -> BuckyResult<()> {
        self.rollback().await.map_err(map_sql_err)
    }

    async fn commit_transaction(mut self) -> BuckyResult<()> {
        self.commit().await.map_err(map_sql_err)
    }
}

#[async_trait]
#[cfg(db_type = "postgres")]
impl<'a> DBTransaction for sqlx::Transaction<'a, sqlx::Postgres>
{
    async fn rollback_transaction(mut self) -> BuckyResult<()> {
        self.rollback().await.map_err(map_sql_err)
    }

    async fn commit_transaction(mut self) -> BuckyResult<()> {
        self.commit().await.map_err(map_sql_err)
    }
}

#[cfg(test)]
mod test_connection {
    use crate::{DBConnection, map_sql_err, DBTransaction, DBExecutor, DBQuery, DBQueryAs};
    use cyfs_base::BuckyResult;
    use sqlx::{SqliteConnection, Connection, Row};

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
        Ok(DB::Connection::connect("sqlite::memory:").await.map_err(map_sql_err)?)
    }

    #[test]
    fn test() {
        async_std::task::block_on(async {
            let mut sqlx_conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
            let mut tx = sqlx_conn.begin_transaction().await.unwrap();
            let create_table = r#"CREATE TABLE IF NOT EXISTS desc_extra (
            "obj_id" char(45) PRIMARY KEY NOT NULL UNIQUE,
        "rent_arrears" INTEGER,
        "rent_arrears_count" INTEGER,
        "rent_value" INTEGER,
        "coin_id" INTEGER,
        "data_len" INTEGER,
        "other_charge_balance" INTEGER);"#;
            tx.execute_sql(create_table).await.unwrap();
            let insert = r#"insert into desc_extra (obj_id,
            rent_arrears,
            rent_arrears_count,
            rent_value,
            coin_id,
            data_len,
            other_charge_balance) values (
            "test", 1, 1, 2, 3, 4, 5)"#;
            tx.execute_sql(insert).await.unwrap();
            tx.commit_transaction().await.unwrap();

            let query = sqlx::query("select * from desc_extra where obj_id = ?").bind("test");
            let row = sqlx_conn.query_one(query).await.unwrap();
            let id: String = row.get("obj_id");
            assert_eq!(id, "test".to_owned());
            let coin_id: i32 = row.get("coin_id");
            assert_eq!(coin_id, 3);

            let row = sqlx::query("select * from desc_extra where obj_id = ?").bind("test").query_one(&mut sqlx_conn).await.unwrap();
            let id: String = row.get("obj_id");
            assert_eq!(id, "test".to_owned());
            let coin_id: i32 = row.get("coin_id");
            assert_eq!(coin_id, 3);

            let query = sqlx::query_as::<_, DescExtra>("select * from desc_extra where obj_id = ?").bind("test").query_one(&mut sqlx_conn).await.unwrap();
            assert_eq!(query.obj_id, "test".to_owned());
            assert_eq!(query.coin_id, 3);
        })
    }
}
