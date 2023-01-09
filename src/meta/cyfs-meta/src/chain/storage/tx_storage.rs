use std::path::{PathBuf};
use cyfs_base::*;
use cyfs_base_meta::*;
use crate::{MetaConnection, MetaConnectionOptions, map_sql_err, DBExecutor};
use sqlx::{ConnectOptions, Row, SqlitePool};
use log::LevelFilter;
use std::time::Duration;
use sqlx::sqlite::SqliteJournalMode;

pub struct TxStorage {
    db_path: PathBuf,
    pool: SqlitePool
}


impl TxStorage {
    pub async fn new(path: PathBuf) -> BuckyResult<Self> {
        let mut options = MetaConnectionOptions::new().filename(path.as_path()).create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory).busy_timeout(Duration::new(10, 0));
        options.log_statements(LevelFilter::Off).log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
        let storage = Self {
            db_path: path,
            pool: SqlitePool::connect_lazy_with(options)
        };
        storage.init().await?;
        Ok(storage)
    }

    async fn get_conn(&self, _read_only: bool) -> BuckyResult<MetaConnection> {
        self.pool.acquire().await.map_err(map_sql_err)
    }

    async fn init(&self) -> BuckyResult<()> {
        static INIT_TX_TBL_SQL: &str = "CREATE TABLE IF NOT EXISTS \"tx\"(
            \"hash\" CHAR(64) PRIMARY KEY NOT NULL UNIQUE,
            \"number\" INTEGER NOT NULL,\
            \"_index\" INTEGER NOT NULL)";
        let mut conn = self.get_conn(false).await?;
        conn.execute_sql(sqlx::query(INIT_TX_TBL_SQL)).await?;
        Ok(())
    }

    pub async fn add_block(&self, block: &Block) -> BuckyResult<()> {
        static INSERT_TX_SQL: &str = "INSERT INTO tx (hash, number, _index) VALUES (?1, ?2, ?3)";
        let mut conn = self.get_conn(false).await?;
        let mut index = 0;
        for tx in block.transactions() {
            conn.execute_sql(sqlx::query(INSERT_TX_SQL)
                .bind(tx.desc().calculate_id().to_string())
                .bind(block.header().number())
                .bind(index)).await?;
            index += 1;
        }
        Ok(())
    }

    pub async fn get_tx_seq(&self, tx_hash: &TxHash) -> BuckyResult<(i64, i64)> {
        static QUERY_TX_SQL: &str = "SELECT number, _index FROM tx WHERE hash=?1";
        let mut conn = self.get_conn(true).await?;
        let row = conn.query_one(sqlx::query(QUERY_TX_SQL).bind(tx_hash.to_string())).await?;

        let number: i64 = row.get("number");
        let index: i64 = row.get("_index");
        Ok((number, index))
    }
}
