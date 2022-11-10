//#![windows_subsystem = "windows"]
#![recursion_limit = "256"]

mod facade_logger;
mod bench;
mod sim_zone;
mod stat;
mod dec_service;
use std::sync::Arc;

use clap::{App, Arg};
use cyfs_lib::SharedCyfsStack;
use log::*;
use async_trait::async_trait;
use crate::facade_logger::FacadeLogger;
use crate::bench::*;
use crate::sim_zone::*;
use crate::stat::Stat;
use crate::dec_service::*;

#[macro_use]
extern crate log;

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum BenchEnv {
    Physical,
    Simulator,
}

#[async_trait]
pub trait Bench {
    async fn bench(&self, env: BenchEnv, zone: &SimZone, ood_path: String, times: u64) -> bool;
    fn name(&self) -> &str;
}

#[async_std::main]
async fn main() {
    let matches = App::new("cyfs-stack-bench").version(cyfs_base::get_version()).about("bench cyfs stack")
    .arg(Arg::with_name("save").short("s").long("save").default_value("./cyfs-stack-bench-result.txt"))
    .arg(Arg::with_name("path").short("p").long("path"))
    .arg(Arg::with_name("simulator").long("simulator"))
    .arg(Arg::with_name("physical").long("physical"))
    .arg(Arg::with_name("times").short("t").long("times").default_value("128"))
    .arg(Arg::with_name("dec-service").short("d").long("dec-service"))
    .get_matches();

    let logger = if matches.occurrences_of("save") != 0 {
        FacadeLogger::new(matches.value_of("save"))
    } else {
        FacadeLogger::new(None)
    };

    logger.start().unwrap();

    let benchs: Vec<Box<dyn Bench>> = vec![
        Box::new(NONBench {}),
        Box::new(RootStateBench {}),
        Box::new(NDNBench {}),
        Box::new(TransBench {}),
    ];

    let ood_path = matches.value_of("path").unwrap_or("./");
    let times = matches.value_of("times").unwrap_or_default().parse::<u64>().unwrap_or(1024);
    let _dec_service = matches.value_of("dec-service").unwrap_or_default().parse::<bool>().unwrap_or(false);

    debug!("start benchmark");
    if matches.is_present("simulator") {
        // open 1-device/ 1-ood  1-other ood
        let zone = SimZone::init_zone().await;
        Stat::clear(&zone).await;

        // 切换目录到当前exe的相对目录
        let root = std::env::current_exe().unwrap();
        let root = root.parent().unwrap().join("cyfs");
        std::fs::create_dir_all(&root).unwrap();
        cyfs_util::bind_cyfs_root_path(root);

        for bench in &benchs {
            trace!("************************** SIMULATOR {} ********************************", bench.name());
            debug!("start {} {}", "SIMULATOR", bench.name());
            let ret = bench.bench(BenchEnv::Simulator, &zone, ood_path.to_string(), times).await;
            if !ret {
                error!("{} failed", bench.name());
            }
            trace!("************************** SIMULATOR {} End ********************************", bench.name());
            trace!("");
            if !ret {
                break;
            }
        }

        // 输出统计
        Stat::read(&zone, times).await;
    }

    if matches.is_present("physical") {
        // open 1-device/ 1-ood  1-other ood
        // FIXED: 使用真实协议栈open runtime/ood
        let zone = SimZone::init_zone().await;
        Stat::clear(&zone).await;

        // 切换目录到当前exe的相对目录
        let root = std::env::current_exe().unwrap();
        let root = root.parent().unwrap().join("cyfs");
        std::fs::create_dir_all(&root).unwrap();
        cyfs_util::bind_cyfs_root_path(root);

        for bench in &benchs {
            trace!("************************** REAL STACK {} ********************************", bench.name());
            debug!("start {} {}", "REAL STACK", bench.name());
            let ret = bench.bench(BenchEnv::Physical, &zone, ood_path.to_string(), times).await;
            if !ret {
                error!("{} failed", bench.name());
            }
            trace!("************************** REAL STACK {} End ********************************", bench.name());
            trace!("");
            if !ret {
                break;
            }
        }

        // 输出统计
        Stat::read(&zone, times).await;
    }

    if matches.is_present("dec-service") {
        //使用默认配置初始化non-stack，因为是跑在gateway后面，共享了gateway的协议栈，所以配置使用默认即可
        //FIXME: 这里需要用testservice app的app id来初始化SharedObjectStack
        let cyfs_stack = SharedCyfsStack::open_default(None).await.unwrap();
        let _ = cyfs_stack.online().await;

        log::info!("test-dec-service run as {}", cyfs_stack.local_device_id());
        let mut service = TestService::new(cyfs_stack);
        service.init().await;

        TestService::start(Arc::new(service));
        async_std::task::block_on(async_std::future::pending::<()>());
    }


    std::process::exit(0);

}

#[cfg(test)]
mod main_tests {
    use cyfs_base::*;
    use cyfs_meta_lib::{MetaClient, MetaClientHelper, MetaMinerTarget};
    use std::str::FromStr;

    pub const OBJ_ID: &'static str = "5r4MYfFU8UfvqjN3FPxcf7Ta4gx9c56jrXdNwnVyp29e";
    //cargo test -- --nocapture
    #[async_std::test]
    async fn test_get_meta_object() -> BuckyResult<()> {
        let meta_client = MetaClient::new_target(MetaMinerTarget::from_str("http://127.0.0.1:1423")?)
        .with_timeout(std::time::Duration::from_secs(60 * 2));

        let object_id = ObjectId::from_str(OBJ_ID).unwrap();
        let ret = MetaClientHelper::get_object(&meta_client, &object_id).await?;
        if ret.is_none() {
            let msg = format!(
                "load object from meta chain but not found! id={}",
                object_id
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (_, object_raw) = ret.unwrap();

        // 解码
        let p = People::clone_from_slice(&object_raw).map_err(|e| {
            let msg = format!("decode people object failed! id={}, {}", object_id, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;
        println!("people: {:?}, {:?}", p.name(), p.ood_list());

        Ok(())
    }
}