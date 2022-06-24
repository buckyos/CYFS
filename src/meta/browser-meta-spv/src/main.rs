mod block_monitor;
mod server;
mod storage;
mod status;
mod helper;

use std::path::Path;
use log::*;
use serde::{Deserialize};
use crate::storage::create_storage;
use cyfs_base::BuckyResult;
use std::sync::Arc;
use crate::block_monitor::BlockMonitor;
use crate::status::Status;
use crate::server::SPVServer;


#[derive(Deserialize)]
pub struct Config {
    meta_endpoint: String,
    check_interval: u64,
    engine: String,
    sqlite: Option<SqliteConfig>,
    mysql: Option<MysqlConfig>,
    service_endpoint: String
}

#[derive(Deserialize)]
struct SqliteConfig {
    database_path: String
}

#[derive(Deserialize)]
struct MysqlConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    db: String
}

#[async_std::main]
async fn main() -> BuckyResult<()> {
    cyfs_debug::CyfsLoggerBuilder::new_app("browser-meta-spv")
        .level("info")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("browser-meta-spv", "browser-meta-spv")
        .build()
        .start();
    info!("start spv");

    let config_path = Path::new("./config.toml");
    if !config_path.exists() {
        error!("cannot find config file.");
        std::process::exit(1);
    }

    match toml::from_str::<Config>(std::fs::read_to_string(config_path).unwrap().as_str()) {
        Ok(config) => {
            let status = Arc::new(Status::new());
            let storage = Arc::new(create_storage(&config).await.map_err(|e|{
                error!("create storage err {}", e);
                e
            })?);
            let monitor = Arc::new(BlockMonitor::new(&config.meta_endpoint, storage.clone(), status.clone()));
            monitor.init().await?;
            BlockMonitor::run(monitor.clone(), config.check_interval);
            let mut server = SPVServer::new(&config, storage.clone(), status.clone());
            server.register();
            server.run().await;
        }
        Err(e) => {
            error!("parse config file err {}", e);
            std::process::exit(2);
        }
    }

    return Ok(())
}
