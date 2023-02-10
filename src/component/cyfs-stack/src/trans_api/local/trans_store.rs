use crate::trans_api::{sql_query, SqlConnection, SqlPool};
use cyfs_base::*;
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

    if !base_dir.is_dir() {
        async_std::fs::create_dir_all(&base_dir).await.map_err(|e| {
            let msg = format!("create tracker-cache dir failed! dir={}, {}", base_dir.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;
    }
    
    base_dir.push("trans.db");

    info!("will init trans store: db={}", base_dir.display());

    let store = TransStore::new(base_dir).await?;
    store.init().await.map_err(|e| {
        error!("init trans store failed! {}", e);
        e
    })?;

    Ok(Arc::new(store))
}
