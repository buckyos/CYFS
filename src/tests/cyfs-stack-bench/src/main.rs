//#![windows_subsystem = "windows"]
#![recursion_limit = "256"]

mod bench;
mod stat;
mod post_service;
mod util;

use std::sync::Arc;

use clap::{App, Arg, ArgMatches};
use cyfs_lib::{GlobalStatePathAccessItem, SharedCyfsStack};
use log::*;
use cyfs_base::{AccessPermissions, BuckyError, BuckyErrorCode, BuckyResult, DeviceId, ObjectId};
use crate::bench::*;
use crate::stat::Stat;
use crate::post_service::*;
use cyfs_core::DecAppObj;
use cyfs_debug::LogLevel;
use std::str::FromStr;
mod config;

fn read_config(matches: &ArgMatches) -> BuckyResult<config::Config> {
    if matches.is_present("simulator") {
        Ok(config::Config::simulator())
    } else if matches.is_present("config") {
        let root = std::env::current_exe().unwrap();
        let config_path = root.parent().unwrap().join("config.toml");
        if !config_path.exists() {
            error!("cannot find config file. {}", config_path.display());
            std::process::exit(1);
        }
        match toml::from_str::<config::Config>(std::fs::read_to_string(config_path).unwrap().as_str()) {
            Ok(config) => {
                Ok(config)
            }
            Err(_e) => {
                Err(BuckyError::from(BuckyErrorCode::InvalidFormat))
            }
        }
    } else {
        error!("no config. use --simulator to test on zone-simulator or use --config to specify config path");
        Err(BuckyError::from(BuckyErrorCode::NotFound))
    }
}

// 准备好对应的被动端协议栈，包括注册handler，开放权限等等, 返回这个协议栈的DeviceId
async fn prepare_stack(stack: &SharedCyfsStack) -> DeviceId {
    let _ = stack.online().await;

    let stub = stack.root_state_meta_stub(None, None);
    stub.add_access(GlobalStatePathAccessItem::new_group(NON_OBJECT_PATH, None, None, Some(DEVICE_DEC_ID.clone()), AccessPermissions::ReadAndWrite as u8)).await.unwrap();
    stub.add_access(GlobalStatePathAccessItem::new_group(GLOABL_STATE_PATH, None, None, Some(DEVICE_DEC_ID.clone()), AccessPermissions::ReadAndWrite as u8)).await.unwrap();
    stub.add_access(GlobalStatePathAccessItem::new_group(NDN_CHUNKS_PATH, None, None, Some(DEVICE_DEC_ID.clone()), AccessPermissions::ReadAndWrite as u8)).await.unwrap();

    let service = TestService::new(stack.clone());
    service.start().await;

    stack.local_device_id()
}

#[async_std::main]
async fn main() {
    let matches = App::new("cyfs-stack-bench").version(cyfs_base::get_version()).about("bench cyfs stack")
    .arg(Arg::with_name("simulator").long("simulator"))
    .arg(Arg::with_name("times").short("t").long("times").takes_value(true))
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

    // physical vood env
    if matches.is_present("dec-service") {
        //使用默认配置初始化non-stack，因为是跑在gateway后面，共享了gateway的协议栈，所以配置使用默认即可
        let cyfs_stack = SharedCyfsStack::open_default(Some(OOD_DEC_ID.clone())).await.unwrap();
        let stack_id = prepare_stack(&cyfs_stack).await;
        info!("start bench as service in {}", stack_id);
        async_std::task::block_on(async_std::future::pending::<()>());
    }

    match read_config(&matches) {
        Ok(mut config) => {
            if matches.is_present("simulator") {
                info!("run benchmark on simulator, register service");
                // zone1_ood as server
                let service_stack = SharedCyfsStack::open_with_port(Some(OOD_DEC_ID.clone()), 21000, 21001).await.unwrap();
                let stack_id = prepare_stack(&service_stack).await;
                config.same_zone_target = Some(stack_id.object_id().clone().to_string());
                // zone2_ood as server
                let other_ood_stack = SharedCyfsStack::open_with_port(Some(OOD_DEC_ID.clone()), 21010, 21011).await.unwrap();
                let other_stack_id = prepare_stack(&other_ood_stack).await;
                config.cross_zone_target = Some(other_stack_id.object_id().clone().to_string());
            }

            let run_times = matches.value_of("times").map(|times| {
                times.parse::<usize>().map_err(|e| {
                    error!("input param times {} invalid, use default value 128", times);
                    e
                }).unwrap_or(128)
            }).unwrap_or(config.run_times.unwrap_or(128));
            info!("ood dec_id: {}, device dec_id: {}", OOD_DEC_ID.to_string(), DEVICE_DEC_ID.to_string());
            // device as requestor
            let test_stack = SharedCyfsStack::open_with_port(Some(DEVICE_DEC_ID.clone()), config.http_port, config.ws_port).await.unwrap();
            test_stack.online().await.unwrap();

            let same_zone_target = Some(ObjectId::from_str(config.same_zone_target.unwrap().as_str()).unwrap());
            let cross_zone_target = Some(ObjectId::from_str(config.cross_zone_target.unwrap().as_str()).unwrap());

            benchs.push(SameZoneNONBench::new(test_stack.clone(), same_zone_target.clone(), stat.clone(), run_times));
            benchs.push(CrossZoneNONBench::new(test_stack.clone(), cross_zone_target.clone(), stat.clone(), run_times));
            benchs.push(SameZoneGlobalStateBench::new(test_stack.clone(), same_zone_target.clone(), stat.clone(), run_times));
            benchs.push(CrossZoneRootStateBench::new(test_stack.clone(), cross_zone_target.clone(), stat.clone(), run_times));
            benchs.push(SameZoneRmetaBench::new(test_stack.clone(), same_zone_target.clone(), stat.clone(), run_times));
            benchs.push(SameZoneCryptoBench::new(test_stack.clone(), same_zone_target.clone(), stat.clone(), run_times));

            benchs.push(TransBench::new(test_stack.clone(), cross_zone_target.clone(), stat.clone(), run_times));

            benchs.push(SameZoneNDNBench::new(test_stack.clone(), same_zone_target.clone(), stat.clone(), run_times));
            benchs.push(CrossZoneNDNBench::new(test_stack.clone(), cross_zone_target.clone(), stat.clone(), run_times));

            for bench in &mut benchs {
                info!("begin test {}...", bench.name());
                let begin = std::time::Instant::now();
                let ret = bench.bench().await;
                info!("end test {}, use {:?}", bench.name(), begin.elapsed());
                if ret.is_err() {
                    error!("{} failed", bench.name());
                    break;
                }
            }

            // 输出统计
            stat.print(benchs.as_slice());
        },
        Err(e) => {
            error!("read config error {}", e);
        }
    }
}

lazy_static::lazy_static! {
    static ref DEVICE_DEC_ID: ObjectId = cyfs_core::DecApp::generate_id(ObjectId::default(), "cyfs-stack-bench");
    static ref OOD_DEC_ID: ObjectId = cyfs_core::DecApp::generate_id(ObjectId::default(), "cyfs-stack-bench-2");
}