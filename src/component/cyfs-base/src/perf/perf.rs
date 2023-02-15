use crate::{BuckyErrorCode, BuckyResult};

use once_cell::sync::OnceCell;
use std::sync::Arc;

pub trait PerfIsolate: Send + Sync {
    fn begin_request(&self, id: &str, key: &str);
    fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>);

    fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>);

    fn action(
        &self,
        id: &str,
        err: BuckyErrorCode,
        name: &str,
        value: &str,
    );

    fn record(&self, id: &str, total: u64, total_size: Option<u64>);
}

pub type PerfIsolateRef = Arc<Box<dyn PerfIsolate>>;

#[async_trait::async_trait]
pub trait PerfManager: Send + Sync {
    async fn flush(&self) -> BuckyResult<()>;
    fn get_isolate(&self, id: &str) -> PerfIsolateRef;
}

pub static PERF_MANGER: OnceCell<Box<dyn PerfManager>> = OnceCell::new();
