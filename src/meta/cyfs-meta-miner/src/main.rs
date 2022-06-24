use clap::{App, Arg};
use cyfs_meta::*;
use std::sync::Arc;
use std::path::Path;

#[async_std::main]
async fn main() {
    cyfs_debug::CyfsLoggerBuilder::new_service("meta_miner")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("app", "meta_miner")
        .build()
        .start();

    let matches = App::new("cyfs meta miner").version(cyfs_base::get_version())
        .arg(Arg::with_name("path").short("p").long("path").value_name("PATH").help("set meta chain path.\ndefault is current path.").takes_value(true))
        .arg(Arg::with_name("port").long("port").help("set meta chain service port.\ndefault is 1423.").takes_value(true))
        .get_matches();

    let chain_path = matches.value_of("path").unwrap_or("./");
    let chain_port = matches.value_of("port").unwrap_or(::cyfs_base::CYFS_META_MINER_PORT.to_string().as_str()).parse::<u16>().unwrap_or(::cyfs_base::CYFS_META_MINER_PORT);

    let miner: Arc<dyn Miner> = ChainCreator::start_miner_instance(Path::new(chain_path), new_sql_storage).unwrap();
    let server = MetaHttpServer::new(miner, chain_port);
    server.run().await.unwrap();
}
