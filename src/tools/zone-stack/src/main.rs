#![recursion_limit="256"]

mod profile;
mod loader;

#[macro_use]
extern crate log;

use cyfs_debug::*;
use clap::App;

#[async_std::main]
async fn main() {
    let app = App::new("zone-stack")
        .version(cyfs_base::get_version())
        .about("zone-stack tools for cyfs system")
        .author("CYFS <dev@cyfs.com>");

    let _matches = app.get_matches();

    // 切换目录到当前exe的相对目录
    let root = std::env::current_exe().unwrap();
    let root = root.parent().unwrap().join("cyfs");
    std::fs::create_dir_all(&root).unwrap();
    cyfs_util::bind_cyfs_root_path(root);

    CyfsLoggerBuilder::new_app("zone-stack")
        .level("trace")
        .console("trace")
        .enable_bdt(Some("warn"), Some("warn"))
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("tools", "zone-stack")
        .exit_on_panic(true)
        .build()
        .start();

    async_std::task::block_on(async move {
        if let Err(e) = loader::Loader::load().await {
            error!("{}", e);
            std::process::exit(-1);
        }
    });
    
    async_std::task::sleep(std::time::Duration::from_millis(u64::MAX)).await;
}
