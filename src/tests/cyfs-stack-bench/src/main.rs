//#![windows_subsystem = "windows"]
#![recursion_limit = "256"]

mod facade_logger;
mod bench;
mod loader;
use clap::{App, Arg};
use log::*;
use async_trait::async_trait;
use crate::facade_logger::FacadeLogger;
use crate::bench::*;

#[macro_use]
extern crate log;

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum BenchEnv {
    RealStack,
    Simulator,
}

#[async_trait]
pub trait Bench {
    async fn bench(&self, env: BenchEnv, ood_path: String, throughput: u64) -> bool;
    fn name(&self) -> &str;
}

#[async_std::main]
async fn main() {
    let matches = App::new("cyfs-stack-bench").version(cyfs_base::get_version()).about("bench cyfs stack")
    .arg(Arg::with_name("save").short("s").long("save").default_value("./cyfs-stack-bench-result.txt"))
    .arg(Arg::with_name("path").short("p").long("path"))
    .arg(Arg::with_name("real-stack").long("real-stack"))
    .arg(Arg::with_name("simulator").long("simulator"))
    .arg(Arg::with_name("throughput").short("t").long("throughput").default_value("1024"))
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
    let throughput = matches.value_of("throughput").unwrap_or_default().parse::<u64>().unwrap_or(1024);

    // 切换目录到当前exe的相对目录
    let root = std::env::current_exe().unwrap();
    let root = root.parent().unwrap().join("cyfs");
    std::fs::create_dir_all(&root).unwrap();
    cyfs_util::bind_cyfs_root_path(root);

    if matches.is_present("simulator") {
        loader::load(true).await;

        for bench in &benchs {
            trace!("************************** SIMULATOR {} ********************************", bench.name());
            debug!("start {} {}", "SIMULATOR", bench.name());
            let ret = bench.bench(BenchEnv::Simulator, ood_path.to_string(), throughput).await;
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

    if !matches.is_present("real-stack") {
        loader::load(false).await;
        for bench in &benchs {
            trace!("************************** REAL STACK {} ********************************", bench.name());
            debug!("start {} {}", "REAL STACK", bench.name());
            let ret = bench.bench(BenchEnv::RealStack, ood_path.to_string(), throughput).await;
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

}
