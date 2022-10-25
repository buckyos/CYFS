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
    async fn bench(&self, env: BenchEnv, throughput: u64) -> bool;
    fn name(&self) -> &str;
}

#[async_std::main]
async fn main() {
    let matches = App::new("cyfs-stack-bench").version(cyfs_base::get_version()).about("bench cyfs stack")
    .arg(Arg::with_name("save").short("s").long("save").default_value("./cyfs-stack-bench-result.txt"))
    .arg(Arg::with_name("real-stack").long("real-stack"))
    .arg(Arg::with_name("simulator").long("simulator"))
    .arg(Arg::with_name("throughput").short("t").long("throughput").default_value("1024"))
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
        Box::new(RouterHandlerBench {}),
        Box::new(NDNBench {}),
        Box::new(TransBench {}),
    ];

    let throughput = matches.value_of("throughput").unwrap_or_default().parse::<u64>().unwrap_or(1024);

    if !matches.is_present("simulator") {
        loader::load().await;

        for bench in &benchs {
            trace!("************************** SIMULATOR {} ********************************", bench.name());
            debug!("start {} {}", "SIMULATOR", bench.name());
            let ret = bench.bench(BenchEnv::Simulator, throughput).await;
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

    if matches.is_present("real-stack") {
        for bench in &benchs {
            trace!("************************** REAL STACK {} ********************************", bench.name());
            debug!("start {} {}", "REAL STACK", bench.name());
            let ret = bench.bench(BenchEnv::RealStack, throughput).await;
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
