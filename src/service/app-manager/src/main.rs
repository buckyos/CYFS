#![windows_subsystem = "windows"]
use cyfs_base::APP_MANAGER_NAME;
use cyfs_client::NamedCacheClient;
use cyfs_core::get_system_dec_app;
use log::*;
use cyfs_lib::SharedCyfsStack;
extern crate ood_daemon;
use ood_daemon::init_system_config;

use crate::app_manager_ex::AppManager as AppManagerEx;
use std::sync::Arc;

mod app_cmd_executor;
mod app_controller;
mod app_manager_ex;
mod dapp;
mod docker_api;
mod docker_network_manager;
mod event_handler;
mod non_helper;
mod package;

#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec(APP_MANAGER_NAME);
    //cyfs_base::init_log_with_isolate_bdt(APP_MANAGER_NAME, Some("debug"), None);

    cyfs_debug::CyfsLoggerBuilder::new_service(APP_MANAGER_NAME)
        .level("debug")
        .console("debug")
        .enable_bdt(Some("debug"), Some("debug"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-service", APP_MANAGER_NAME)
        .build()
        .start();

    {
        if let Err(e) = init_system_config().await {
            error!("load system config err: {}", e);
            std::process::exit(1);
        }
    }

    // 初始化named_cache_client，以后整合了bdt的chunk就不需要这个了
    let mut named_cache_client = NamedCacheClient::new();
    named_cache_client.init(None, None, None).await.unwrap();

    // 使用默认配置初始化non-stack，因为是跑在gateway后面，共享了gateway的协议栈，所以配置使用默认即可
    // 兼容gateway没启动的情况，在这里等待gateway启动后再往下走
    let cyfs_stack;
    match SharedCyfsStack::open_default(Some(get_system_dec_app().object_id().clone())).await {
        Ok(stack) => {
            info!("open default stack success");
            cyfs_stack = stack;
        }
        Err(e) => {
            error!("open default stack err, {}", e);
            std::process::exit(1);
        }
    }
    let _ = cyfs_stack.wait_online(None).await;

    //旧逻辑
    // let mut app_manager = AppManager::new(cyfs_stack, named_cache_client);
    // app_manager.init().await;

    // // start内部起定时器，定期检查App状态
    // AppManager::start(Arc::new(app_manager)).await;

    // async_std::task::block_on(async_std::future::pending::<()>());

    //新逻辑
    let mut app_manager = AppManagerEx::new(cyfs_stack, named_cache_client);
    if let Err(e) = app_manager.init().await {
        error!("init app manamger err, {}", e);
        std::process::exit(1);
    }

    // start内部起定时器，定期检查App状态
    AppManagerEx::start(Arc::new(app_manager)).await;

    async_std::task::block_on(async_std::future::pending::<()>());
}
