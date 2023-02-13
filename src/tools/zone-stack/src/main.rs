#![recursion_limit = "256"]

mod loader;
mod profile;

#[macro_use]
extern crate log;

use clap::App;
use cyfs_debug::*;
use zone_simulator::CyfsStackInsConfig;
use cyfs_lib::BrowserSanboxMode;

use std::str::FromStr;
use clap::Arg;

#[async_std::main]
async fn main() {
    let app = App::new("zone-stack")
        .version(cyfs_base::get_version())
        .about("zone-stack tools for cyfs system")
        .author("CYFS <dev@cyfs.com>")
        .arg(
            Arg::with_name("browser-mode")
                .long("browser-mode")
                .takes_value(true)
                .help("The browser sanbox mode, default is none"),
        );

    let matches = app.get_matches();

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

    let browser_mode = match matches.value_of("browser-mode") {
        Some(v) => Some(
            BrowserSanboxMode::from_str(v)
                .map_err(|e| {
                    println!("invalid browser mode param! {}, {}", v, e);
                    std::process::exit(-1);
                })
                .unwrap(),
        ),
        None => None,
    };

    let mut stack_config = CyfsStackInsConfig::default();
    if let Some(mode) = browser_mode {
        stack_config.browser_mode = mode;
    }

    async_std::task::block_on(async move {
        if let Err(e) = loader::Loader::load(stack_config).await {
            error!("{}", e);
            std::process::exit(-1);
        }
    });

    async_std::future::pending::<()>().await;
}
