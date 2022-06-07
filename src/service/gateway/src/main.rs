#![windows_subsystem = "windows"]

mod control;
mod gateway;
mod server;
mod upstream;

#[macro_use]
extern crate log;

use crate::gateway::GATEWAY;
// use acc_service::{Service as AccService, SERVICE_NAME as ACC_SERVICE_NAME};
use cyfs_stack_loader::CyfsServiceLoader;

use clap::{App, Arg};

pub const SERVICE_NAME: &str = ::cyfs_base::GATEWAY_NAME;

async fn main_run() {
    let app = App::new("gateway service")
        .version(cyfs_base::get_version())
        .about("gateway service for cyfs system")
        .author("CYFS <dev@cyfs.com>")
        .arg(
            Arg::with_name("gateway_only")
                .short("g")
                .long("gateway-only")
                .takes_value(false)
                .help("Run gateway service without acc_service"),
        );

    let app = cyfs_util::process::prepare_args(app);
    let matches = app.get_matches();

    cyfs_util::process::check_cmd_and_exec_with_args(SERVICE_NAME, &matches);

    cyfs_debug::CyfsLoggerBuilder::new_service(SERVICE_NAME)
        .level("debug")
        .console("debug")
        .enable_bdt(Some("trace"), Some("debug"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-service", SERVICE_NAME)
        .build()
        .start();

    cyfs_debug::ProcessDeadHelper::instance().enable_exit_on_task_system_dead(None);

    // ::cyfs_base::init_log_with_isolate_bdt(SERVICE_NAME, Some("debug"), Some("trace"));

    // 初始化全局变量管理器
    {
        if let Err(_e) = CyfsServiceLoader::prepare_env().await {
            std::process::exit(-1);
        }
    }

    // gateway核心服务
    if let Err(e) = GATEWAY.lock().unwrap().load_config().await {
        error!("load config failed! err={}", e);
        std::process::exit(-1);
    }

    GATEWAY.lock().unwrap().start();

    control::GatewayControlServer::run().await;
}

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}
