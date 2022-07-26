use super::perf::*;
use cyfs_base::*;

pub struct TracePerf {
    id: String,
}

impl TracePerf {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
        }
    }
}

impl Perf for TracePerf {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    // create a new perf module
    fn fork(&self, id: &str) -> BuckyResult<Box<dyn Perf>> {
        Ok(Box::new(Self { id: id.to_owned() }))
    }

    fn begin_request(&self, id: &str, key: &str) {
        trace!("[perf] begin_request: id={}, key={}", id, key);
    }

    fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        trace!(
            "[perf] end_request: id={}, key={}, err={}, bytes={:?}",
            id,
            key,
            err,
            bytes
        );
    }

    fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        trace!("[perf] acc: id={}, err={}, size={:?}", id, err, size);
    }

    fn action(&self, id: &str, err: BuckyErrorCode, name: String, value: String) {
        trace!(
            "[perf] action: id={}, err={}, name={}, value={}",
            id,
            err,
            name,
            value
        );
    }

    fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        trace!(
            "[perf] record: id={}, total={}, total_size={:?}",
            id,
            total,
            total_size
        );
    }
}
