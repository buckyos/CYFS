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

use once_cell::sync::OnceCell;
use std::sync::Mutex;

pub struct PerfHolder {
    id: String,
    perf: OnceCell<Box<dyn Perf>>,
    children: Mutex<Vec<PerfHolder>>,
}

impl PerfHolder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            perf: OnceCell::new(),
            children: Mutex::new(vec![]),
        }
    }

    pub fn bind(&self, perf: Box<dyn Perf>) {
        let list = self.children.lock().unwrap();
        list.iter().for_each(|holder| match perf.fork(&holder.id) {
            Ok(perf) => {
                holder.bind(perf);
            }
            Err(e) => {
                error!("fork perf error! id={}, {}", holder.id, e);
            }
        });

        if let Err(_) = self.perf.set(perf) {
            unreachable!();
        }
    }

    fn fork(&self, id: impl Into<String>) -> BuckyResult<PerfHolder> {
        let new_item = Self::new(id);

        if let Some(perf) = self.perf.get() {
            let perf = perf.fork(&new_item.id)?;
            new_item.bind(perf);
        }

        Ok(new_item)
    }

    pub fn get(&self) -> Option<&Box<dyn Perf>> {
        self.perf.get()
    }
}

pub type PerfHolderRef = Arc<PerfHolder>;

pub struct PerfTrace {
    id: String,
}

impl Perf for PerfTrace {
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
        trace!("[perf] end_request: id={}, key={}, err={}, bytes={:?}", id, key, err, bytes);
    }

    fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        trace!("[perf] acc: id={}, err={}, size={:?}", id, err, size);
    }

    fn action(&self, id: &str, err: BuckyErrorCode, name: String, value: String) {
        trace!("[perf] action: id={}, err={}, name={}, value={}", id, err, name, value);
    }

    fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        trace!("[perf] record: id={}, total={}, total_size={:?}", id, total, total_size);
    }
}
