#![windows_subsystem = "windows"]

mod chunk_context;
mod chunk_daemon;
mod chunk_delegate;
mod chunk_interface;
mod chunk_manager;
mod chunk_meta;
mod chunk_processor;
mod chunk_store;
mod chunk_tx;
mod gateway_helper;

#[macro_use]
extern crate log;

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    cyfs_util::process::check_cmd_and_exec(cyfs_base::CHUNK_MANAGER_NAME);
    //::cyfs_base::init_log("chunk-manager", None);

    cyfs_debug::CyfsLoggerBuilder::new_service(cyfs_base::CHUNK_MANAGER_NAME)
        .level("debug")
        .console("debug")
        .enable_bdt(Some("debug"), Some("debug"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-service", cyfs_base::CHUNK_MANAGER_NAME)
        .build()
        .start();

    let chunk_dir = ::cyfs_util::get_cyfs_root_path().join("data/chunk-manager/chunk");
    let ret = std::fs::create_dir_all(&chunk_dir);
    if let Err(e) = ret {
        error!(
            "create chunk dir failed, err:{}, chunk dir:{}",
            e,
            chunk_dir.into_os_string().into_string().unwrap()
        );
        return Err(std::io::Error::from(std::io::ErrorKind::Other));
    }

    gateway_helper::register();

    info!("chunk_dir is:{}", chunk_dir.to_str().unwrap());
    let database = ::cyfs_util::get_cyfs_root_path().join("data/chunk-manager/chunk.index");
    let mut interface = chunk_interface::ChunkInterface::new(&database);
    interface.init(&chunk_dir).await.map_err(|e| {
        error!(
            "init failed, chunk_dir is:{}, err:{}",
            chunk_dir.to_str().unwrap(),
            e
        );
        std::io::Error::from(std::io::ErrorKind::Interrupted)
    })?;
    interface.run().await?;

    Ok(())
}
