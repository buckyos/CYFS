mod contract_service;
mod cache_service;
use std::sync::Arc;
use std::time::Duration;
use config::builder::DefaultState;
use config::ConfigBuilder;
use cyfs_lib::*;
use cyfs_util::process::ProcessAction;
use cyfs_dsg_client::*;
use contract_service::*;
use cyfs_util::get_app_data_dir;


fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();
    async_std::task::block_on(main_run());
}

async fn main_run() {
    let status = cyfs_util::process::check_cmd_and_exec("cyfs dsg service");
    if status == ProcessAction::Install {
        std::process::exit(0);
    }

    cyfs_debug::CyfsLoggerBuilder::new_app("cyfs dsg service")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("off"), Some("off"))
        .module("cyfs-lib", Some("off"), Some("off"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs dsg", "cyfs dsg service")
        .build()
        .start();

    let mut config_builder = ConfigBuilder::<DefaultState>::default()
        .set_default("challenge_interval", 24*3600).unwrap()
        .set_default("initial_challenge_live_time", 24*3600).unwrap()
        .set_default("store_challenge_live_time", 3600).unwrap();
    let data_dir = get_app_data_dir("cyfs dsg service");
    let config_path = data_dir.join("config.toml");
    if config_path.exists() {
        let file = config::File::from(config_path.as_path());
        config_builder = config_builder.add_source(file);
    }
    let config = config_builder.build().unwrap();

    let stack = SharedCyfsStack::open_default(Some(dsg_dec_id()))
        .await
        .unwrap();

    let mut dsg_config = DsgServiceConfig::default();
    dsg_config.challenge_interval = Duration::from_secs(config.get_int("challenge_interval").unwrap() as u64);
    dsg_config.initial_challenge.live_time = Duration::from_secs(config.get_int("initial_challenge_live_time").unwrap() as u64);
    dsg_config.store_challenge.live_time = Duration::from_secs(config.get_int("store_challenge_live_time").unwrap() as u64);
    let _service = DsgService::new(Arc::new(stack), dsg_config)
        .await
        .unwrap();

    async_std::task::block_on(async_std::future::pending::<()>());
}
