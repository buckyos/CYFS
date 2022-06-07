use clap::{App, Arg};
use cyfs_base::{BuckyResult};
use cyfs_base_meta::*;
use cyfs_meta_spv::{BlockMonitor, SPVChainStorage, SPVHttpServer};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

#[macro_use]
extern crate log;

#[derive(Serialize, Deserialize)]
struct Config {
    meta_host: String,
    port: u16,
}

#[async_std::main]
async fn main() -> BuckyResult<()> {
    cyfs_debug::CyfsLoggerBuilder::new_service("cyfs-meta-spv")
        .level("info")
        .console("info")
        .enable_bdt(Some("off"), Some("off"))
        .module("cyfs-lib", Some("off"), Some("off"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-meta-spv", "cyfs-meta-spv")
        .build()
        .start();

    let matches = App::new("cyfs meta spv")
        .version(cyfs_base::get_version())
        .arg(
            Arg::with_name("path")
                .short("p")
                .long("path")
                .value_name("PATH")
                .help("set spv path.\ndefault is current path.")
                .takes_value(true),
        )
        .get_matches();

    let chain_path = matches.value_of("path").unwrap_or("./");

    let config_path = Path::new(chain_path).join("config.json");
    let config_file = File::open(config_path.as_path())
        .map_err(|err| {
            error!(
                "open config.json at {} failed, err {}",
                config_path.display(),
                err
            );
            ERROR_NOT_FOUND
        })
        .unwrap();
    let config: Config = serde_json::from_reader(config_file)
        .map_err(|err| {
            error!("invalid config.json, err {}", err);
            ERROR_PARAM_ERROR
        })
        .unwrap();

    let storage = SPVChainStorage::load(Path::new(chain_path)).await.unwrap();

    let monitor = BlockMonitor::new(config.meta_host.as_str(), storage.clone());
    monitor.run().await;

    let server = SPVHttpServer::new(storage, config.port);
    server.run().await.unwrap();

    Ok(())
}
