use crate::trans_api::{sql_query, SqlConnection, SqlPool};
use cyfs_base::BuckyResult;
use std::path::Path;
use std::sync::Arc;

pub struct TransStore {
    pool: SqlPool,
}

impl TransStore {
    pub async fn new<P: AsRef<Path>>(store_path: P) -> BuckyResult<Self> {
        let pool = SqlPool::open(
            format!(
                "sqlite://{}",
                store_path.as_ref().to_string_lossy().to_string()
            )
            .as_str(),
            5,
        )
        .await?;
        Ok(Self { pool })
    }

    pub async fn init(&self) -> BuckyResult<()> {
        let mut conn = self.pool.get_conn().await?;
        let sql = r#"create table if not exists "download_task_tracker" (
            "source" char(45) not null,
            "dec_id" char(45) not null,
            "context_id" char(45) not null,
            "task_id" char(45) not null,
            "task_status" INTEGER,
            "created_at" TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            "updated_at" TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create index if not exists dec_index on download_task_tracker (source, dec_id, context_id, task_status, created_at, task_id)"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create index if not exists dec_index2 on download_task_tracker (source, dec_id, task_status, created_at, task_id)"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create index if not exists dec_index3 on download_task_tracker (source, dec_id, created_at, task_id)"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create index if not exists task_index on download_task_tracker (task_id, source, dec_id)"#;
        conn.execute_sql(sql_query(sql)).await?;

        Ok(())
    }

    pub async fn create_connection(&self) -> BuckyResult<SqlConnection> {
        self.pool.get_conn().await
    }
}

pub async fn create_trans_store(isolate: &str) -> BuckyResult<Arc<TransStore>> {
    let mut base_dir = cyfs_util::get_cyfs_root_path();
    base_dir.push("data");
    base_dir.push(isolate);
    base_dir.push("tracker-cache");
    base_dir.push("trans.db");

    let store = TransStore::new(base_dir).await?;
    store.init().await?;
    Ok(Arc::new(store))
}
