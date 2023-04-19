#![windows_subsystem = "windows"]
mod chunk_interface;
mod chunk_store;
mod gateway_helper;

use cyfs_core::get_system_dec_app;
use cyfs_lib::SharedCyfsStack;
use crate::chunk_store::ChunkStore;
use log::*;

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    cyfs_util::process::check_cmd_and_exec(cyfs_base::CHUNK_MANAGER_NAME);

    cyfs_debug::CyfsLoggerBuilder::new_service(cyfs_base::CHUNK_MANAGER_NAME)
        .level("info")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-service", cyfs_base::CHUNK_MANAGER_NAME)
        .build()
        .start();

    let stack = SharedCyfsStack::open_default(Some(get_system_dec_app().clone())).await.unwrap();
    let chunk_dir = ::cyfs_util::get_cyfs_root_path().join("data/chunk-manager/chunk");
    if chunk_dir.exists() {
        info!("find old chunk data dir {}, merge into stack", chunk_dir.display());
        if let Err(e) = ChunkStore::merge(&chunk_dir, stack.clone()).await {
            error!("merge old chunk data err {}, try re-merge at next startup", e);
        }
    }

    gateway_helper::register();
    let database = ::cyfs_util::get_cyfs_root_path().join("data/chunk-manager/chunk.index");
    if database.exists() {
        info!("remove old database file {}", database.display());
        let _ = std::fs::remove_file(&database);
    }
    let interface = chunk_interface::ChunkInterface::new(stack);
    interface.run().await?;

    Ok(())
}
