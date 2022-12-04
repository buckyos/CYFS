#![recursion_limit = "256"]

mod loader;
mod profile;
mod user;
mod zone;

#[macro_use]
extern crate log;

use cyfs_debug::*;
use loader::*;

use clap::{App, Arg};


async fn main_run() {
    let default_root = cyfs_util::default_cyfs_root_path();
    let cyfs_root_help = format!("Specify cyfs root dir, default is {}", default_root.display());

    let app = App::new("zone-simulator")
        .version(cyfs_base::get_version())
        .about("zone-simulator tools for cyfs system")
        .author("CYFS <dev@cyfs.com>")
        .arg(
            Arg::with_name("dump")
                .short("d")
                .long("dump")
                .takes_value(false)
                .help("Dump all desc/sec files to {cyfs}/etc/zone-simulator"),
        )
        .arg(
            Arg::with_name("random_mnemonic")
                .short("r")
                .long("random")
                .takes_value(false)
                .help("Generate random random mnemonic"),
        ).arg(
            Arg::with_name("cyfs_root")
                .long("cyfs-root")
                .takes_value(true)
                .help(&cyfs_root_help),
        );

    let matches = app.get_matches();

    let random_mnemonic = matches.is_present("random_mnemonic");
    if random_mnemonic {
        loader::random_mnemonic();
        std::process::exit(0);
    }

    if let Some(cyfs_root) = matches.value_of("cyfs_root") {
        cyfs_util::bind_cyfs_root_path(cyfs_root);
    }

    let dump = matches.is_present("dump");

    CyfsLoggerBuilder::new_app("zone-simulator")
        .level("trace")
        .console("trace")
        .enable_bdt(Some("warn"), Some("warn"))
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("tools", "zone-simulator")
        .exit_on_panic(true)
        .build()
        .start();

    // 首先加载助记词
    profile::TEST_PROFILE.load();

    let (user1, user2) = TestLoader::load_users(profile::TEST_PROFILE.get_mnemonic(), true, dump).await;
    profile::TEST_PROFILE.save_desc();

    if dump {
        std::process::exit(0);
    }

    TestLoader::load_stack(user1, user2).await;

    async_std::future::pending::<u8>().await;
}

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run());
}