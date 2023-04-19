use std::path::{Path};
use std::str::FromStr;
use async_std::prelude::StreamExt;
use log::LevelFilter;
use sqlx::{ConnectOptions, Executor, Pool, Row, Sqlite};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use cyfs_base::{BuckyResult, ObjectId, AccessString};
use cyfs_lib::{NONPutObjectOutputRequest, SharedCyfsStack};

pub struct FileManager {
    database: Pool<Sqlite>,
}

const SELECT: &str = r#"
    SELECT id, desc from file_desc;
"#;

impl FileManager {
    pub async fn merge(database: &Path, stack: SharedCyfsStack) -> BuckyResult<()> {
        let mut options = SqliteConnectOptions::new().filename(database).create_if_missing(false).read_only(true);
        options.log_statements(LevelFilter::Off);
        let pool = SqlitePoolOptions::new().max_connections(10).connect_with(options).await?;

        let mut stream = pool.fetch(SELECT);
        while let Some(row) = stream.next().await {
            let row = row?;
            let id: String = row.try_get("id")?;
            let desc: Vec<u8> = row.try_get("desc")?;
            match ObjectId::from_str(&id) {
                Ok(id) => {
                    let mut request = NONPutObjectOutputRequest::new_noc(id, desc);
                    request.access = Some(AccessString::full());
                    match stack.non_service().put_object(request).await {
                        Ok(resp) => {
                            info!("insert obj {} to stack result {}", &id, resp.result.to_string());
                        }
                        Err(e) => {
                            error!("insert obj {} to stack err {}, skip", &id, e);
                        }
                    }
                }
                Err(e) => {
                    error!("decode object id {} err {}, skip it", &id, e);
                }
            }
        }

        info!("insert all object to stack complete, delete database file {}", database.display());

        pool.close().await;

        std::fs::remove_file(database)?;

        Ok(())
    }
}