mod contract_service;
mod cache_service;
use std::sync::Arc;
use std::time::Duration;
use config::builder::DefaultState;
use config::ConfigBuilder;
use cyfs_lib::*;
use cyfs_base::*;
use cyfs_util::process::ProcessAction;
use cyfs_dsg_client::*;
use contract_service::*;
use cyfs_util::get_app_data_dir;


fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();
    async_std::task::block_on(main_run());
}

async fn main_run() {
    const NAME: &str = "cyfs-dsg-service";
    let status = cyfs_util::process::check_cmd_and_exec(NAME);
    if status == ProcessAction::Install {
        std::process::exit(0);
    }

    cyfs_debug::CyfsLoggerBuilder::new_service(NAME)
        .level("debug")
        .console("debug")
        .enable_bdt(Some("debug"), Some("debug"))
        .module("cyfs-lib", Some("debug"), Some("debug"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new(NAME, NAME)
        .build()
        .start();

    let mut config_builder = ConfigBuilder::<DefaultState>::default()
        .set_default("challenge_interval", 24*3600).unwrap()
        .set_default("initial_challenge_live_time", 24*3600).unwrap()
        .set_default("store_challenge_live_time", 3600).unwrap();
    let data_dir = get_app_data_dir(NAME);
    let config_path = data_dir.join("config.toml");
    if config_path.exists() {
        let file = config::File::from(config_path.as_path());
        config_builder = config_builder.add_source(file);
    }
    let config = config_builder.build().unwrap();

    let dec_id = dsg_dec_id();
    log::info!("----> dec id # {}", &dec_id);
    let stack = SharedCyfsStack::open_default(Some(dec_id))
        .await
        .unwrap();
    stack.wait_online(None).await.unwrap();

    let path = RequestGlobalStatePath::new(None, Some("/dmc/dsg/miner/")).format_string();
    stack.root_state_meta_stub(None, None).add_access(GlobalStatePathAccessItem {
        path: path.clone(),
        access: GlobalStatePathGroupAccess::Default(AccessString::full().value()),
    }).await.unwrap();

    let mut dsg_config = DsgServiceConfig::default();
    dsg_config.challenge_interval = Duration::from_secs(config.get_int("challenge_interval").unwrap() as u64);
    dsg_config.initial_challenge.live_time = Duration::from_secs(config.get_int("initial_challenge_live_time").unwrap() as u64);
    dsg_config.store_challenge.live_time = Duration::from_secs(config.get_int("store_challenge_live_time").unwrap() as u64);
    let _service = DsgService::new(Arc::new(stack), dsg_config)
        .await
        .unwrap();

    async_std::task::block_on(async_std::future::pending::<()>());
}
