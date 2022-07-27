use cyfs_base::*;

use std::sync::Arc;

pub trait Perf: Send + Sync {
    fn get_id(&self) -> String;

    // create a new perf module
    fn fork(&self, id: &str) -> BuckyResult<Box<dyn Perf>>;

    // 开启一个request
    fn begin_request(&self, id: &str, key: &str);

    // 统计一个操作的耗时 or 流量统计
    fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>);

    fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>);

    fn action(&self, id: &str, err: BuckyErrorCode, name: String, value: String);

    fn record(&self, id: &str, total: u64, total_size: Option<u64>);
}

pub type PerfRef = Arc<Box<dyn Perf>>;