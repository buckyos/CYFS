mod contract_service;
mod cache_service;
use std::sync::Arc;
use cyfs_lib::*;
use cyfs_util::process::ProcessAction;
use cyfs_dsg_client::*;
use contract_service::*;



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

    let stack = SharedCyfsStack::open_default(Some(dsg_dec_id()))
        .await
        .unwrap();

    let _service = DsgService::new(Arc::new(stack), DsgServiceConfig::default())
        .await
        .unwrap();

    async_std::task::block_on(async_std::future::pending::<()>());
}
