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
        let ret = loader::load(true).await;
        if let Err(e) = ret {
            error!("Failed to load sim stack, {}", e);
            std::process::exit(-1);
        }
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

    if matches.is_present("real-stack") {
        let ret = loader::load(false).await;
        if let Err(e) = ret {
            error!("Failed to load sim stack, {}", e);
            std::process::exit(-1);
        }
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