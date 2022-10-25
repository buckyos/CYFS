mod facade_logger;
mod bench;

use clap::{App, Arg};
use log::*;
use async_trait::async_trait;
use crate::facade_logger::FacadeLogger;
use crate::bench::*;

#[async_trait]
pub trait Bench {
    async fn bench(&self, throughput: u64) -> bool;
    fn name(&self) -> &str;
}

#[async_std::main]
async fn main() {
    let matches = App::new("cyfs-stack-bench").version(cyfs_base::get_version()).about("bench cyfs stack")
    .arg(Arg::with_name("save").short("s").long("save").default_value("./cyfs-stack-bench-result.txt"))
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
        Box::new(NDNBench {}),
        Box::new(TransBench {}),
    ];

    let throughput = matches.value_of("throughput").unwrap_or_default().parse::<u64>().unwrap_or(1024);

    for bench in &benchs {
        trace!("************************** STACK {} ********************************", bench.name());
        debug!("start {} {}", "STACK", bench.name());
        let ret = bench.bench(throughput).await;
        if !ret {
            error!("{} failed", bench.name());
        }
        trace!("************************** STACK {} End ********************************", bench.name());
        trace!("");
        if !ret {
            break;
        }
    }

}
