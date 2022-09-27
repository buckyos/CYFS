#![recursion_limit = "256"]

mod case;
mod loader;

#[macro_use]
extern crate log;

use clap::{App, Arg};

use cyfs_debug::*;

#[async_std::main]
async fn main() {
    let matches = App::new("cyfs-stack-case").version(cyfs_base::get_version()).about("cyfs stack testcase")
    .arg(Arg::with_name("end_time").long("end_time"))
    .get_matches();

    CyfsLoggerBuilder::new_app("cyfs-stack-case")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("error"), Some("error"))
        .disable_file_config(true)
        .file(true)
        .build()
        .unwrap()
        .start();

    let end_time = matches.value_of("end_time").unwrap_or_default().parse::<u64>().unwrap_or(60 * 5);

    PanicBuilder::new("tools", "cyfs-stack-case")
        .exit_on_panic(true)
        .build()
        .start();

    loader::load().await;

    case::test().await;

    info!("test process now will exits!");
    async_std::task::sleep(std::time::Duration::from_secs(end_time)).await;
    std::process::exit(0);
}