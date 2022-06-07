#![recursion_limit = "256"]

mod container;
mod stack;

#[macro_use]
extern crate log;

use cyfs_debug::*;
use container::CyfsContainerManager;
use cyfs_util::bind_cyfs_root_path;

pub const SERVICE_NAME: &str = "cyfs-container";

#[async_std::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count: u16 = args.get(1).unwrap().parse().unwrap();
    #[cfg(target_os = "windows")]
    {
        bind_cyfs_root_path("c:\\cyfs_container");
    }
    #[cfg(any(target_os = "linux"))]
    {
        bind_cyfs_root_path("/cyfs_container");
    }

    CyfsLoggerBuilder::new_service(SERVICE_NAME)
        .level("debug")
        .console("warn")
        .enable_bdt(Some("debug"), Some("warn"))
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("cyfs-sdk", SERVICE_NAME).build().start();
    warn!("container start");
    let container = CyfsContainerManager::new(count);
    if let Err(e) = container.start().await {
        error!("cyfs container init failed: {}", e);
        let code: u16 = e.code().into();
        std::process::exit(code as i32);
    }

    async_std::future::pending::<u8>().await;
}
