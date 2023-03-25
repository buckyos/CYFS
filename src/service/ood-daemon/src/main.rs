#![windows_subsystem = "windows"]

mod config;
mod config_repo;
mod daemon;
mod monitor;
mod package;
mod repo;
mod service;
mod status;

use clap::{App, Arg};
use std::str::FromStr;

use daemon::{start_control, Daemon};
use service::{ServiceMode, SERVICE_MANAGER};
use cyfs_util::HttpInterfaceHost;

#[macro_use]
extern crate log;

const SERVICE_NAME: &str = ::cyfs_base::OOD_DAEMON_NAME;

fn start_log() {
    cyfs_debug::CyfsLoggerBuilder::new_service(SERVICE_NAME)
        .level("info")
        .console("info")
        .enable_bdt(Some("info"), Some("info"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-service", SERVICE_NAME)
        .build()
        .start();
}

async fn main_run() {
    let app = App::new("ood-daemon service")
        .version(cyfs_base::get_version())
        .about("ood-daemon service for cyfs system")
        .author("CYFS <dev@cyfs.com>")
        .arg(
            Arg::with_name("as_monitor")
                .long("as-monitor")
                .takes_value(false)
                .help("Run ood-daemon service as backend monitor service"),
        )
        .arg(
            Arg::with_name("no_monitor")
                .long("no-monitor")
                .takes_value(false)
                .help("Run ood-daemon service without backend monitor service"),
        )
        .arg(
            Arg::with_name("no_ood_control")
                .long("no-ood-control")
                .takes_value(false)
                .help("Run ood-daemon service without ood control service"),
        )
        .arg(
            Arg::with_name("mode")
                .long("mode")
                .takes_value(true)
                .default_value("daemon")
                .help("Daemon service mode, daemon|installer|vood, default is daemon"),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .takes_value(true)
                .help("Specify OOD bind service port"),
        )
        .arg(
            Arg::with_name("host")
                .long("host")
                .takes_value(true)
                .help("Specify OOD service public address for external services and tools, installer will bind 0 addr by default"),
        )
        .arg(
            Arg::with_name("strict-host")
                .long("strict-host")
                .takes_value(true)
                .help("Specify OOD bind service public address"),
        )
        .arg(
            Arg::with_name("ipv4_only")
                .long("ipv4-only")
                .takes_value(false)
                .help("Specify OOD bind service just use ipv4 address"),
        )
        .arg(
            Arg::with_name("ipv6_only")
                .long("ipv6-only")
                .takes_value(false)
                .help("Specify OOD bind service just use ipv6 address"),
        ).arg(
            Arg::with_name("startup_mode")
                .long("startup-mode")
                .takes_value(false)
                .help("Start the service when on system startup"),
        ).arg(
            Arg::with_name("stop_all")
                .long("stop-all")
                .alias("stop_all")
                .takes_value(false)
                .help("Stop all the services include ood-daemon"),
        ).arg(
            Arg::with_name("status_host")
                .long("status-host")
                .takes_value(true)
                .help("Specify the http address of the status service, which can be local/unspecified/a list of ip addresses separated by commas, and the default is local"),
        );

    let app = cyfs_util::process::prepare_args(app);
    let matches = app.get_matches();

    let as_monitor = matches.is_present("as_monitor");
    if as_monitor {
        monitor::ServiceMonitor::run_as_monitor(SERVICE_NAME);
        return;
    }

    let no_monitor = matches.is_present("no_monitor");

    // 如果是stop命令，那么也需要尝试停止monitor进程
    if !no_monitor && matches.is_present("stop") {
        monitor::ServiceMonitor::stop_monitor_process(SERVICE_NAME);
    }

    if matches.is_present("stop_all") {
        start_log();

        let code = match crate::daemon::ServicesStopController::new()
            .stop_all()
            .await
        {
            Ok(()) => 0,
            Err(e) => {
                let code: u16 = e.code().into();
                code as i32
            }
        };

        std::process::exit(code);
    }

    cyfs_util::process::check_cmd_and_exec_with_args(SERVICE_NAME, &matches);

    start_log();

    // ::cyfs_base::init_log_with_isolate_bdt(SERVICE_NAME, Some("trace"), Some("trace"));

    // 启动monitor服务
    if !no_monitor {
        if let Err(e) = monitor::ServiceMonitor::start_monitor(SERVICE_NAME) {
            error!("start monitor failed! {}", e);
        }
    }

    if matches.is_present("startup_mode") {
        verify_state_on_startup();
    }

    // 切换到目标的服务模式
    let mode = matches.value_of("mode").unwrap();
    let mode = match ServiceMode::from_str(mode) {
        Ok(v) => v,
        Err(_) => {
            return;
        }
    };
    SERVICE_MANAGER.change_mode(mode.clone());

    let no_ood_control = matches.is_present("no_ood_control");

    if !no_ood_control {
        if let Err(e) = start_control(mode.clone(), &matches).await {
            println!("start ood control failed! {}", e);
            std::process::exit(e.code().into());
        }
    } else {
        info!("will run without ood control service");
    }

    let status_host = if let Some(host) = matches.value_of("status_host") {
        HttpInterfaceHost::from_str(host)
            .map_err(|e| {
                println!("invalid status-host param! {}, {}", host, e);
                std::process::exit(e.code().into());
            })
            .unwrap()
    } else {
        HttpInterfaceHost::default()
    };

    let daemon = Daemon::new(mode, no_monitor);
    if let Err(e) = daemon.run(status_host).await {
        error!("daemon run error! err={}", e);
        std::process::exit(e.code().into());
    }
}

fn verify_state_on_startup() {
    for _ in 0..3 {
        match cyfs_util::init_system_hosts() {
            Ok(ret) => {
                if !ret.none_local_ip_v4.is_empty() {
                    break;
                }

                warn!("none local ip_v4 address is empty! now will try with some wait...");
            }
            Err(e) => {
                // FIXME should exit process now?
                error!("get system hosts failed! {}", e);
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}
