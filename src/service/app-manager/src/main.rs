#![windows_subsystem = "windows"]
use crate::app_controller::AppController;
use crate::app_manager_ex::AppManager as AppManagerEx;
use app_manager_lib::{AppManagerConfig};
use clap::{App, Arg};
use cyfs_base::*;
use cyfs_core::DecAppId;
use cyfs_lib::SharedCyfsStack;
use cyfs_util::process::{
    prepare_args, set_process_cmd_funcs, ProcessAction, ProcessCmdFuncs,
};
use log::*;
use ood_daemon::init_system_config;
use std::fs;
use std::{str::FromStr, sync::Arc};

mod app_acl_util;
mod app_cmd_executor;
mod app_controller;
mod app_install_detail;
mod app_manager_ex;
mod dapp;
mod docker_api;
mod docker_network_manager;
mod event_handler;
mod non_helper;
mod package;

struct AppManagerProcessFuncs {
    config: AppManagerConfig,
}

impl AppManagerProcessFuncs {
    async fn stop_apps(&self) -> BuckyResult<()> {
        let app_controller = AppController::new(self.config.clone(), ObjectId::default());

        let app_dir = cyfs_util::get_cyfs_root_path().join("app");
        info!("[STOP] stop all apps");

        for entry in fs::read_dir(&app_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(dir_name) = path.file_name() {
                if let Some(app_id_str) = dir_name.to_str() {
                    if let Ok(app_id) = DecAppId::from_str(app_id_str) {
                        let ret = app_controller.stop_app(&app_id).await;
                        info!("[STOP] stop app:{}, ret:{:?}", app_id, ret);
                    }
                }
            }
        }

        Ok(())
    }
}

impl ProcessCmdFuncs for AppManagerProcessFuncs {
    fn exit_process(&self, action: ProcessAction, code: i32) -> ! {
        if action == ProcessAction::Stop {
            let _ = async_std::task::block_on(self.stop_apps());
        }

        info!("exit process, action:{:?}, code:{}", action, code);
        std::process::exit(code);
    }
}

async fn main_run() {
    //cyfs_base::init_log_with_isolate_bdt(APP_MANAGER_NAME, Some("debug"), None);
    //let action = cyfs_util::process::check_cmd_and_exec(APP_MANAGER_NAME);

    let app = App::new(&format!("{}", APP_MANAGER_NAME)).version(cyfs_base::get_version());

    let app = prepare_args(app);
    app.arg(Arg::with_name("sync-repo"))
    let matches = app.get_matches();
    let (action, _) = cyfs_util::process::parse_cmd(APP_MANAGER_NAME, &matches);
    if action == ProcessAction::Start || action == ProcessAction::Stop {
        cyfs_debug::CyfsLoggerBuilder::new_service(APP_MANAGER_NAME)
            .level("debug")
            .console("debug")
            .enable_bdt(Some("debug"), Some("debug"))
            .build()
            .unwrap()
            .start();

        cyfs_debug::PanicBuilder::new("cyfs-service", APP_MANAGER_NAME)
            .exit_on_panic(true)
            .build()
            .start();
    }

    let app_config = AppManagerConfig::load();

    info!("app manager use docker:{}", app_config.use_docker());

    let _ = set_process_cmd_funcs(Box::new(AppManagerProcessFuncs { config: app_config.clone() }));

    cyfs_util::process::check_cmd_and_exec_with_args(APP_MANAGER_NAME, &matches);

    if action == ProcessAction::Stop {
        unreachable!("Stop cmd should exit.");
    }

    {
        if let Err(e) = init_system_config().await {
            error!("load system config err: {}", e);
            std::process::exit(1);
        }
    }

    // 使用默认配置初始化non-stack，因为是跑在gateway后面，共享了gateway的协议栈，所以配置使用默认即可
    // 兼容gateway没启动的情况，在这里等待gateway启动后再往下走
    let cyfs_stack;
    match SharedCyfsStack::open_default(Some(cyfs_core::get_system_dec_app().clone())).await {
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

    let mut app_manager = AppManagerEx::new(cyfs_stack, app_config);

    if let Err(e) = app_manager.init().await {
        error!("init app manamger err, {}", e);
        std::process::exit(1);
    }

    AppManagerEx::start(Arc::new(app_manager)).await;

    async_std::task::block_on(async_std::future::pending::<()>());
}

/*fn kill_service_start_process(service_name: &str) -> bool {
    let s = System::new_all();
    for process in s.processes_by_name(service_name) {
        let pid = process.pid();
        info!("find process: {} {} {:?}", pid, process.name(), process.cmd());
        let mut match_count = 0;
        for s in process.cmd() {
            if s.find("cyfs").is_some() && s.find("services").is_some() {
                match_count += 1;
            }
            if s == "--start" {
                match_count += 1;
            }
        }
        if match_count == 2 {
            let ret = process.kill();
            info!("find {} process, pid: {}, kill result:{}", service_name, pid, ret);
            return ret
        }
    }

    return false;
}*/

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}
