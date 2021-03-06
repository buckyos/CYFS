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

#[async_std::main]
async fn main() {
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
        );

    let matches = app.get_matches();

    let random_mnemonic = matches.is_present("random_mnemonic");
    if random_mnemonic {
        loader::random_mnemonic();
        std::process::exit(0);
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

    async_std::task::sleep(std::time::Duration::from_millis(u64::MAX)).await;
}
