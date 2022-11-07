//#![windows_subsystem = "windows"]
#![recursion_limit = "256"]

mod facade_logger;
mod bench;
mod sim_zone;
use clap::{App, Arg};
use log::*;
use async_trait::async_trait;
use crate::facade_logger::FacadeLogger;
use crate::bench::*;
use crate::sim_zone::*;

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
    .arg(Arg::with_name("times").short("t").long("times").default_value("1024"))
    .get_matches();

    // TODO: desc-tool 生成一套身份device ood1, ood2, 内嵌三个协议栈
    // bench times ex 1k

    let logger = if matches.occurrences_of("save") != 0 {
        FacadeLogger::new(matches.value_of("save"))
    } else {
        FacadeLogger::new(None)
    };

    logger.start().unwrap();

    let benchs: Vec<Box<dyn Bench>> = vec![
        Box::new(NONBench {}),
        Box::new(RootStateBench {}),
        Box::new(RouterHandlerBench {}),
        Box::new(NDNBench {}),
        Box::new(TransBench {}),
    ];

    let ood_path = matches.value_of("path").unwrap_or("./");
    let times = matches.value_of("times").unwrap_or_default().parse::<u64>().unwrap_or(1024);

    debug!("start benchmark");
    if matches.is_present("simulator") {
        // open 1-device/ 1-ood  1-other ood
        let zone = SimZone::init_zone().await;

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
    }

    if matches.is_present("physical") {
        // open 1-device/ 1-ood  1-other ood
        let zone = SimZone::init_zone().await;

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
    }

    std::process::exit(0);

}
