
mod service;
mod storage;
mod config;

use std::path::Path;
use std::str::FromStr;
use service::*;
use async_std::sync::Arc;
use clap::{App, Arg};
use log::{error, info, warn};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use crate::config::{PerfConfig, StackType};

fn parse_config(config_path: &Path) -> BuckyResult<PerfConfig> {
    if !config_path.exists() {
        warn!("config path {} not exists.", config_path.display());
        return Err(BuckyError::from(BuckyErrorCode::NotFound));
    }

    toml::from_str(std::fs::read_to_string(config_path)?.as_str()).map_err(|e| {
        BuckyError::new(BuckyErrorCode::InvalidFormat, e.to_string())
    })?
}

#[async_std::main]
async fn main() {
    let matches = App::new("perf-service").version(cyfs_base::get_version())
        .arg(Arg::with_name("stack").short("s").long("stack").help("set stack mode, replace the config file one"))
        .arg(Arg::with_name("config").short("c").long("config").takes_value(true).default_value("perf-service.toml"))
        .get_matches();

    cyfs_debug::CyfsLoggerBuilder::new_service("perf-service")
        .level("info")
        .console("info")
        .enable_bdt(Some("info"), Some("info"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("ood-service", "perf-service").build().start();

    let config_path = Path::new(matches.value_of("config").unwrap());
    let mut config = match parse_config(config_path) {
        Ok(config) => config,
        Err(e) => {
            warn!("parse config path {} err {}, use default", config_path.display(), e);
            PerfConfig::default()
        }
    };

    if let Some(stack_str) = matches.value_of("stack") {
        info!("get stack param {}", stack_str);
        match StackType::from_str(stack_str) {
            Ok(stack) => {
                config.stack_type = stack;
            }
            Err(e) => {
                error!("parse stack param {} err {}", stack_str, e);
                std::process::exit(1);
            }
        }
    }

    info!("use final config: \n{}", toml::to_string_pretty(&config).unwrap());

    let service = PerfService::create(config).await.unwrap();

    PerfService::start(Arc::new(service));

    async_std::task::block_on(async_std::future::pending::<()>());
}