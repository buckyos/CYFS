//#![windows_subsystem = "windows"]
#![recursion_limit = "256"]

mod bench;
mod stat;
mod post_service;
mod util;

use std::sync::Arc;

use clap::{App, Arg, ArgMatches};
use cyfs_lib::{SharedCyfsStack};
use log::*;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use crate::bench::*;
use crate::stat::Stat;
use crate::post_service::*;
use cyfs_core::DecAppObj;
use cyfs_debug::LogLevel;
mod config;

fn read_config(matches: &ArgMatches) -> BuckyResult<config::Config> {
    if matches.is_present("simulator") {
        Ok(config::Config::simulator())
    } else if let Some(config_path) = matches.value_of("config") {
        Ok(toml::from_slice(&std::fs::read(config_path)?).map_err(|e| {
            error!("load config {} err {}", config_path, e);
            BuckyError::from(BuckyErrorCode::InvalidFormat)
        })?)
    } else {
        error!("no config. use --simulator to test on zone-simulator or use --config to specify config path");
        Err(BuckyError::from(BuckyErrorCode::NotFound))
    }
}

#[async_std::main]
async fn main() {
    let matches = App::new("cyfs-stack-bench").version(cyfs_base::get_version()).about("bench cyfs stack")
    .arg(Arg::with_name("simulator").long("simulator"))
    .arg(Arg::with_name("times").short("t").long("times"))
    .arg(Arg::with_name("dec-service").short("d").long("dec-service"))
    .arg(Arg::with_name("config").short("c").long("config"))
    .get_matches();

    cyfs_debug::CyfsLoggerBuilder::new_service("cyfs-stack-bench")
        .level("info")
        .console("info")
        .disable_module(vec!["cyfs_lib"], LogLevel::Error)
        .build()
        .unwrap()
        .start();

    debug!("start benchmark");
    let stat = Arc::new(Stat::new());

    let mut benchs: Vec<Box<dyn Bench>> = vec![];

    if matches.is_present("dec-service") {
        //使用默认配置初始化non-stack，因为是跑在gateway后面，共享了gateway的协议栈，所以配置使用默认即可
        let cyfs_stack = SharedCyfsStack::open_default(Some(DEC_ID.clone())).await.unwrap();
        let _ = cyfs_stack.online().await;

        info!("start bench as service in {}", cyfs_stack.local_device_id());
        let service = TestService::new(cyfs_stack);

        service.start();
        async_std::task::block_on(async_std::future::pending::<()>());
    }

    match read_config(&matches) {
        Ok(mut config) => {
            if matches.is_present("simulator") {
                info!("run benchmark on simulator, register service");
                let service_stack = SharedCyfsStack::open_with_port(Some(DEC_ID.clone()), 21000, 21001).await.unwrap();
                service_stack.online().await.unwrap();

                let target_id = service_stack.local_device_id().object_id().clone();
                let service = TestService::new(service_stack);
                service.start();

                config.target = Some(target_id);
            }

            let run_times = matches.value_of("times").map(|times| {
                times.parse::<usize>().map_err(|e| {
                    error!("input param times {} invalid, use default value 128", times);
                    e
                }).unwrap_or(128)
            }).unwrap_or(config.run_times.unwrap_or(128));

            let test_stack = SharedCyfsStack::open_with_port(Some(DEC_ID.clone()), config.http_port, config.ws_port).await.unwrap();
            test_stack.online().await.unwrap();

            benchs.push(NONBench::new(test_stack.clone(), config.target.clone(), stat.clone(), run_times));

            for bench in &mut benchs {
                debug!("start {} {}", "SIMULATOR", bench.name());
                let ret = bench.bench().await;
                if ret.is_err() {
                    error!("{} failed", bench.name());
                    break;
                }
            }

            // 输出统计
            stat.print();
        },
        Err(e) => {
            error!("read config error {}", e);
        }
    }
}

lazy_static::lazy_static! {
    static ref DEC_ID: ObjectId = cyfs_core::DecApp::generate_id(ObjectId::default(), "cyfs-stack-bench");
}