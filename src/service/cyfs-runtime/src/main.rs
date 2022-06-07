//#![windows_subsystem = "windows"]
#![recursion_limit = "256"]

#[macro_use]
extern crate log;

mod anonymous;
mod file_cache;
mod mime;
mod proxy;
mod runtime;
mod stack;

use cyfs_debug::*;
use stack::PROXY_PORT;

use clap::{App, Arg};

pub const SERVICE_NAME: &str = ::cyfs_base::CYFS_RUNTIME_NAME;

fn service_mutex_name() -> String {
    match std::env::current_exe() {
        Ok(path) => {
            let hash = cyfs_base::hash_data(path.display().to_string().as_bytes()).to_string();
            format!("{}-{}", SERVICE_NAME, &hash[..12])
        }
        Err(e) => {
            println!("call current_exe failed: {}", e);
            SERVICE_NAME.to_owned()
        }
    }
}

async fn main_run() {
    let proxy_port_help = format!(
        "Specify cyfs-runtime proxy service's local port, default is {}",
        PROXY_PORT
    );
    let app = App::new("cyfs-runtime service")
        .version(cyfs_base::get_version())
        .about("runtime service for cyfs system")
        .author("CYFS <dev@cyfs.com>")
        .arg(
            Arg::with_name("anonymous")
                .long("anonymous")
                .takes_value(false)
                .help("Run cyfs-runtime service with anonymous identity"),
        )
        .arg(
            Arg::with_name("random-id")
                .long("random-id")
                .takes_value(false)
                .help("Run cyfs-runtime service with an new random anonymous identity"),
        )
        .arg(
            Arg::with_name("proxy-port")
                .long("proxy-port")
                .takes_value(true)
                .help(&proxy_port_help),
        );

    let app = cyfs_util::process::prepare_args(app);
    let matches = app.get_matches();

    // 切换root目录
    match dirs::data_dir() {
        Some(dir) => {
            let dir = dir.join("cyfs");
            info!("will use user data dir: {}", dir.display());
            cyfs_util::bind_cyfs_root_path(dir);
        }
        None => {
            error!("get user data dir failed!");
        }
    };

    let root = cyfs_util::get_cyfs_root_path();
    if !root.is_dir() {
        if let Err(e) = std::fs::create_dir_all(&root) {
            error!("create root dir failed! dir={}, err={}", root.display(), e);
            std::process::exit(-1);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // mutex在不同用户有独立的命名空间
        cyfs_util::process::check_cmd_and_exec_with_args(SERVICE_NAME, &matches);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let service_mutex_name = service_mutex_name();
        println!("service mutex name: {}", service_mutex_name);

        cyfs_util::process::check_cmd_and_exec_with_args_ext(
            SERVICE_NAME,
            &service_mutex_name,
            &matches,
        );
    }

    CyfsLoggerBuilder::new_service(SERVICE_NAME)
        .level("info")
        .console("info")
        .enable_bdt(Some("info"), Some("info"))
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("cyfs-sdk", SERVICE_NAME).build().start();

    cyfs_debug::ProcessDeadHelper::instance().enable_exit_on_task_system_dead(None);

    let anonymous = matches.is_present("anonymous");
    let random_id = matches.is_present("random-id");
    let proxy_port = matches.value_of("proxy-port");
    let proxy_port = match proxy_port {
        Some(v) => v
            .parse()
            .map_err(|e| {
                println!("invalid proxy-port value: {}", e);
                std::process::exit(-1);
            })
            .unwrap(),
        None => PROXY_PORT,
    };

    let stack_config = stack::CyfsStackInsConfig {
        is_mobile_stack: false,
        anonymous,
        random_id,
        proxy_port,
    };

    async_std::task::spawn(async {
        let mut runtime = runtime::CyfsRuntime::new(stack_config);
        if let Err(e) = runtime.start().await {
            error!("cyfs runtime init failed: {}", e);
            let code: u16 = e.code().into();
            std::process::exit(code as i32);
        }
    });

    async_std::future::pending::<u8>().await;
}

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}
