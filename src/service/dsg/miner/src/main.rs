mod default;
mod miner;

use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::process::ProcessAction;
use default::*;
use dsg_client::*;
use miner::*;
use std::sync::Arc;

#[async_std::main]
async fn main() {
    let status = cyfs_util::process::check_cmd_and_exec("cyfs dsg service");
    if status == ProcessAction::Install {
        std::process::exit(0);
    }

    cyfs_debug::CyfsLoggerBuilder::new_app("cyfs dsg service")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("off"), Some("off"))
        .module("cyfs-lib", Some("off"), Some("off"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs dsg", "cyfs dsg service")
        .build()
        .start();

    let stack = Arc::new(
        SharedCyfsStack::open_default(Some(DsgDefaultMinerClient::dec_id()))
            .await
            .unwrap(),
    );

    let _service = DsgMiner::new(
        stack.clone(),
        DsgDefaultMiner::new(stack, DsgDefaultMinerConfig::default())
            .await
            .unwrap(),
    )
    .await
    .unwrap();

    async_std::task::block_on(async_std::future::pending::<()>());
}
