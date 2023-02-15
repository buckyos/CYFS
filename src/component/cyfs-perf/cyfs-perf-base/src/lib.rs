mod codec;
mod items;
mod perf;
mod merge;

#[macro_use]
extern crate lazy_static;

use cyfs_base::ObjectId;
use std::str::FromStr;

pub use items::*;
pub use perf::*;
pub use merge::*;

lazy_static! {
    pub static ref PERF_DEC_ID: ObjectId = ObjectId::from_str("9tGpLNnAAYE9Dd4ooNiSjtP5MeL9CNLf9Rxu6AFEc12M").unwrap();
}

pub static PERF_REPORT_PATH: &str = "/.perf/report";