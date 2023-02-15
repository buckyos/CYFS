mod codec;
mod crypto;
mod base;
mod objects;
mod perf;

pub use base::endpoint;

/*
#[deprecated(
    //since = "0.2.1",
    note = "Please use cyfs_base::* or cyfs_base::codec::* instead"
)]
pub use codec as raw_encode;
*/

pub use self::crypto::*;
pub use base::*;
pub use codec::*;
pub use cyfs_base_derive::*;
pub use objects::*;
pub use perf::*;

#[macro_use]
extern crate log;


fn str_to_level(level: &str) -> log::LevelFilter {
    use log::LevelFilter;

    match level {
        "none" => LevelFilter::Off,
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Debug,
    }
}

// only use in test cases!
pub fn init_simple_log(_service_name: &str, log_level: Option<&str>) {
    let level = str_to_level(log_level.unwrap_or("info"));

    if let Err(e) = simple_logger::SimpleLogger::new().with_level(level).init() {
        println!("init simple log error! {}", e);
    }
}
