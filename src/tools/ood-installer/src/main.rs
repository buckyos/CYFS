#![recursion_limit = "256"]

#[macro_use]
extern crate log;


mod asset;
mod control;
mod desc_gen;
mod desc_upload;
mod ood_daemon_init;
mod repo_downloader;
#[cfg(unix)]
mod sys_service;
mod system_config_gen;

use asset::*;
use clap::{App, Arg, ArgMatches};
use desc_gen::DeviceDescGenerator;
use desc_upload::DescUploader;
use ood_daemon_init::{DaemonEnv, OodDaemonInit};
use std::path::PathBuf;
use std::str::FromStr;
use system_config_gen::SystemConfigGen;

async fn run_bind(matches: &ArgMatches<'_>) -> ! {
    use std::net::IpAddr;

    let tcp_port = matches.value_of("port").map(|v| {
        let port: u16 = v
            .parse()
            .map_err(|e| {
                let msg = format!("invalid port: {}, {}", v, e);
                println!("{}", msg);
                std::process::exit(-1);
            })
            .unwrap();
        port
    });

    let tcp_host = matches.value_of("host").map(|v| {
        let addr = IpAddr::from_str(v)
            .map_err(|e| {
                let msg = format!("invalid host: {}, {}", v, e);
                println!("{}", msg);
                std::process::exit(-1);
            })
            .unwrap();
        addr
    });

    let strict_tcp_host = matches.value_of("strict-host").map(|v| {
        let addr = IpAddr::from_str(v)
            .map_err(|e| {
                let msg = format!("invalid strict-host: {}, {}", v, e);
                println!("{}", msg);
                std::process::exit(-1);
            })
            .unwrap();
        addr
    });

    let host = if strict_tcp_host.is_some() {
        Some(ood_control::ControlTCPHost::Strict(
            strict_tcp_host.unwrap(),
        ))
    } else if tcp_host.is_some() {
        Some(ood_control::ControlTCPHost::Default(tcp_host.unwrap()))
    } else {
        None
    };

    let addr_type = if matches.is_present("ipv4_only") {
        ood_control::ControlInterfaceAddrType::V4
    } else if matches.is_present("ipv6_only") {
        ood_control::ControlInterfaceAddrType::V6
    } else {
        ood_control::ControlInterfaceAddrType::All
    };

    if let Err(e) = control::ActivateControl::run(tcp_port, host, addr_type).await {
        println!("run OOD bind service failed! {}", e);
        std::process::exit(-1);
    }

    println!("run OOD bind service success!");
    std::process::exit(0);
}

#[async_std::main]
async fn main() {
    let matches = App::new("ood installer tools")
        .version(cyfs_base::get_version())
        .about("ood installer tools for ffs system")
        .author("CYFS <cyfs@buckyos.com>")
        .arg(
            Arg::with_name("force")
                .short("f")
                .long("force")
                .takes_value(false)
                .help("Overwrite current device.desc or device.sec if already exists"),
        )
        .arg(
            Arg::with_name("local")
                .long("local")
                .takes_value(false)
                .help("Local init device.desc and device.sec if not exists, or will use ood-daemon's remote init as default"),
        )
        .arg(
            Arg::with_name("target")
                .long("target")
                .takes_value(true)
                .default_value("default")
                .help("System target, default/synology/vood/solo"),
        )
        .arg(
            Arg::with_name("no_start")
                .long("no-start")
                .takes_value(false)
                .help("Don't start ood-daemon service, default is yes"),
        )
        .arg(
            Arg::with_name("no_cyfs_repo")
                .long("no-cyfs-repo")
                .takes_value(false)
                .help("Don't extract cyfs_repo.desc, default is yes"),
        )
        .arg(
            Arg::with_name("no_app_repo")
                .long("no-app-repo")
                .takes_value(false)
                .help("Don't extract app_repo.desc, default is yes"),
        )
        .arg(
            Arg::with_name("root")
                .long("root")
                .takes_value(true)
                .help(&format!("Specify cyfs root folder, default is {}", cyfs_util::get_cyfs_root_path().display())),
        )
        .arg(
            Arg::with_name("sync_repo")
                .long("sync-repo")
                .takes_value(false)
                .help(&format!("Sync service packages from repo to local repo store")),
        )
        .arg(
            Arg::with_name("bind")
                .long("bind")
                .takes_value(false)
                .help(&format!("Run OOD bind service")),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .takes_value(true)
                .help(&format!("Specify OOD bind service port, default is {}", cyfs_base::OOD_INSTALLER_CONTROL_PORT)),
        )
        .arg(
            Arg::with_name("host")
                .long("host")
                .takes_value(true)
                .help(&format!("Specify OOD service public address for external services and tools, installer will bind 0 addr by default")),
        )
        .arg(
            Arg::with_name("strict-host")
                .long("strict-host")
                .takes_value(true)
                .help(&format!("Specify OOD bind service public address")),
        )
        .arg(
            Arg::with_name("ipv4_only")
                .long("ipv4-only")
                .takes_value(false)
                .help(&format!("Specify OOD bind service just use ipv4 address")),
        )
        .arg(
            Arg::with_name("ipv6_only")
                .long("ipv6-only")
                .takes_value(false)
                .help(&format!("Specify OOD bind service just use ipv6 address")),
        )
        .get_matches();

    cyfs_util::process::check_cmd_and_exec_with_args("ood-installer", &matches);

    // ????????????????????????
    let bind = matches.is_present("bind");

    let mut disable_file_config = false;
    let console_level = if bind {
        disable_file_config = true;
        "off"
    } else {
        "info"
    };
    cyfs_debug::CyfsLoggerBuilder::new_app("ood-installer")
        .level("info")
        .console(console_level)
        .enable_bdt(Some("info"), Some("debug"))
        .disable_file_config(disable_file_config)
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-tools", "ood-installer")
        .build()
        .start();

    // ???????????????root?????????????????????
    if let Some(v) = matches.value_of("root") {
        let root = PathBuf::from_str(v).unwrap_or_else(|e| {
            error!("invalid root path: root={}, {}", v, e);
            std::process::exit(-1);
        });

        if !root.is_dir() {
            std::fs::create_dir_all(&root).unwrap_or_else(|e| {
                error!("mkdir for root path error: root={}, {}", root.display(), e);
                std::process::exit(-1);
            });
        }

        info!("root dir is {}", root.display());

        cyfs_util::bind_cyfs_root_path(root);
    }

    if bind {
        run_bind(&matches).await;
    }

    let mut force = matches.is_present("force");
    let local = matches.is_present("local");

    if local {
        let mut gen = DeviceDescGenerator::new();
        if gen.exists() {
            warn!(
                "device.desc & device.sec already exists! now will ignore generate and upload to meta"
            );
            if let Err(_e) = gen.load() {
                info!("load device.desc failed! now will reinit device.desc & device.sec!");
                force = true;
            }
        }
        // ??????device.desc??????????????????????????????????????????????????????????????????
        if !gen.exists() || force {
            if let Err(_e) = gen.init(force).await {
                std::process::exit(-1);
            }
        } else {
            info!(
                "device.desc & device.sec already exists: id={}",
                gen.get_device_id()
            );
        }
        let uploader = DescUploader::new();
        if let Err(_e) = uploader.upload().await {
            std::process::exit(-1);
        }
    }

    // ??????target
    let target = matches.value_of("target").unwrap();
    let target = match InstallTarget::from_str(target) {
        Ok(v) => v,
        Err(_) => {
            std::process::exit(-1);
        }
    };

    info!("current target: {}", target);
    let asset = OODAsset::new(&target);
    let no_cyfs_repo = matches.is_present("no_cyfs_repo");
    let no_app_repo = matches.is_present("no_app_repo");
    if let Err(_e) = asset.extract(no_cyfs_repo, no_app_repo) {
        std::process::exit(-1);
    }

    let config_gen = SystemConfigGen::new(&target);
    if let Err(_e) = config_gen.gen() {
        std::process::exit(-1);
    }
    // ?????????system_config+device_config???sync-repo????????????ood-daemon?????????????????????
    {
        let env = DaemonEnv::new(&target);
        if let Err(e) = env.prepare().await {
            error!("init daemon env error! {}", e);
            std::process::exit(-1);
        }
    }

    // ???????????????sync-repo???????????????????????????repo
    if matches.is_present("sync_repo") {
        if let Err(_e) = repo_downloader::RepoDownloader::new().load().await {
            std::process::exit(-1);
        }
    }

    let mut ood_daemon_init = OodDaemonInit::new();
    if let Err(_e) = ood_daemon_init.init().await {
        std::process::exit(-1);
    }

    let no_start = matches.is_present("no_start");
    if !no_start {
        if let Err(_e) = ood_daemon_init.start() {
            std::process::exit(-1);
        }
    }

    if target == InstallTarget::Default {
        #[cfg(unix)]
        {
            if let Err(e) = sys_service::SysService::init() {
                warn!("init system service failed! err={}", e);
            }
        }
    }

    info!("init ood finished!!!");
    std::process::exit(0);
}
