#![windows_subsystem = "windows"]

mod control;
mod gateway;
mod server;
mod upstream;

#[macro_use]
extern crate log;

use crate::gateway::Gateway;
use cyfs_stack_loader::CyfsServiceLoader;
use cyfs_lib::BrowserSanboxMode;

use std::str::FromStr;
use clap::{App, Arg};

pub const SERVICE_NAME: &str = ::cyfs_base::GATEWAY_NAME;

async fn main_run() {
    let app = App::new("gateway service")
        .version(cyfs_base::get_version())
        .about("gateway service for cyfs system")
        .author("CYFS <dev@cyfs.com>")
        .arg(
            Arg::with_name("browser-mode")
                .long("browser-mode")
                .takes_value(true)
                .help("The browser sanbox mode, default is strict"),
        );

    
    let app = cyfs_util::process::prepare_args(app);

    let matches = app.get_matches();

    cyfs_util::process::check_cmd_and_exec_with_args(SERVICE_NAME, &matches);

    cyfs_debug::CyfsLoggerBuilder::new_service(SERVICE_NAME)
        .level("debug")
        .console("debug")
        .enable_bdt(Some("info"), Some("info"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-service", SERVICE_NAME)
        .exit_on_panic(true)
        .build()
        .start();

    cyfs_debug::ProcessDeadHelper::instance().enable_exit_on_task_system_dead(None);

    // ::cyfs_base::init_log_with_isolate_bdt(SERVICE_NAME, Some("debug"), Some("trace"));

    let browser_mode = match matches.value_of("browser-mode") {
        Some(v) => {
            Some(BrowserSanboxMode::from_str(v).map_err(|e| {
                println!("invalid browser mode param! {}, {}", v, e);
                std::process::exit(e.code().into());
            }).unwrap())
        }
        None => {
            None
        }
    };

    let mut stack_config = gateway::CyfsStackInsConfig::default();
    if let Some(mode) = browser_mode {
        stack_config.browser_mode = mode;
    }

    // 初始化全局变量管理器
    {
        if let Err(e) = CyfsServiceLoader::prepare_env().await {
            std::process::exit(e.code().into());
        }
    }

    let gateway = Gateway::new(stack_config);

    // gateway核心服务
    if let Err(e) = gateway.load_config().await {
        error!("load cyfs stack failed! err={}", e);
        std::process::exit(e.code().into());
    }

    gateway.start();

    if let Err(e) = gateway.run().await {
        std::process::exit(e.code().into());
    }
}

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}
