use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_perf_base::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;

// 用以辅助request的区间统计
struct PerfRequestHolder {
    pub pending_reqs: HashMap<String, PerfRequestBeginAction>,
    pub reqs: HashMap<String, PerfRequest>,
}

impl std::fmt::Display for PerfRequestHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "reqs len: {}", self.reqs.len())
    }
}

impl PerfRequestHolder {
    pub fn new() -> Self {
        Self {
            pending_reqs: HashMap::new(),
            reqs: HashMap::new(),
        }
    }

    pub fn begin(&mut self, id: &str, key: &str) {
        let full_id = format!("{}_{}", id, key);

        match self.pending_reqs.entry(full_id) {
            Entry::Vacant(v) => {
                let action = PerfRequestBeginAction {
                    tick: bucky_time_now(),
                };
                v.insert(action);
            }
            Entry::Occupied(_o) => {
                unreachable!("perf request item already begin! id={}, key={}", id, key);
            }
        }
    }

    pub fn end(&mut self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        let now = bucky_time_now();
        let full_id = format!("{}_{}", id, key);
        match self.pending_reqs.remove(&full_id) {
            Some(action) => {
                let during = if now > action.tick {
                    now - action.tick
                } else {
                    0
                };

                match self.reqs.entry(id.to_owned()) {
                    Entry::Vacant(v) => {
                        let mut req = PerfRequest {
                            id: id.to_owned(),
                            time_range: PerfTimeRange {
                                begin: action.tick,
                                end: now,
                            },
                            total: 1,
                            success: 0,
                            total_time: during,
                            total_size: None,
                        };

                        if err == BuckyErrorCode::Ok {
                            req.success = 1;
                        }
                        if let Some(bytes) = bytes {
                            req.total_size = Some(bytes as u64);
                        }

                        v.insert(req);
                    }
                    Entry::Occupied(mut o) => {
                        let item = o.get_mut();
                        item.total += 1;
                        if err == BuckyErrorCode::Ok {
                            item.success += 1;
                        }
                        item.total_time += during;
                        if let Some(bytes) = bytes {
                            match item.total_size.as_mut() {
                                Some(s) => *s += bytes as u64,
                                None => item.total_size = Some(bytes as u64),
                            }
                        }
                    }
                }
            }
            None => {
                //unreachable!();
            }
        }
    }
}

impl PerfItemMerge<PerfRequestHolder> for PerfRequestHolder {
    fn merge(&mut self, other: Self) {
        self.reqs.merge(other.reqs)
    }
}

// 一个统计实体
struct PerfIsolateImpl {
    id: String,

    time_range: PerfTimeRange,

    actions: Vec<PerfAction>,

    records: HashMap<String, PerfRecord>,

    accumulations: HashMap<String, PerfAccumulation>,

    reqs: PerfRequestHolder,
}

impl PerfIsolateImpl {
    pub fn new(id: &str) -> PerfIsolateImpl {
        Self {
            id: id.to_owned(),
            time_range: PerfTimeRange::now(),
            actions: vec![],
            records: HashMap::new(),
            accumulations: HashMap::new(),
            reqs: PerfRequestHolder::new(),
        }
    }

    pub fn begin_request(&mut self, id: &str, key: &str) {
        self.reqs.begin(id, key);
    }

    pub fn end_request(&mut self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        self.reqs.end(id, key, err, bytes);
        self.time_range.update();
    }

    pub fn acc(&mut self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        self.time_range.update();
        match self.accumulations.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let acc = PerfAccumulation {
                    id: id.to_owned(),
                    time_range: PerfTimeRange::now(),
                    success: if err == BuckyErrorCode::Ok { 1 } else { 0 },
                    total: 1,
                    total_size: size,
                };
                v.insert(acc);
            }
            Entry::Occupied(mut o) => {
                let acc = o.get_mut();
                acc.total += 1;
                acc.time_range.end = bucky_time_now();
                if err == BuckyErrorCode::Ok {
                    acc.success += 1;
                }
                if let Some(total) = size {
                    if acc.total_size.is_none() {
                        acc.total_size = Some(total);
                    } else {
                        *acc.total_size.as_mut().unwrap() += total;
                    }
                }
            }
        }
    }

    pub fn action(
        &mut self,
        id: &str,
        err: BuckyErrorCode,
        name: impl Into<String>,
        value: impl Into<String>,
    ) {
        let action = PerfAction {
            id: id.to_owned(),
            time: bucky_time_now(),
            err: err.into(),
            name: name.into(),
            value: value.into(),
        };

        self.actions.push(action);
        self.time_range.update();
    }

    pub fn record(&mut self, id: &str, total: u64, total_size: Option<u64>) {
        let record = PerfRecord {
            id: id.to_owned(),
            time: bucky_time_now(),
            total,
            total_size,
        };

        self.records.insert(id.to_owned(), record);
        self.time_range.update();
    }

    // 取走所有已有的统计项
    pub fn take_data(&mut self) -> PerfIsolateEntity {
        let mut other = PerfIsolateEntity::new(&self.id);

        std::mem::swap(&mut self.time_range, &mut other.time_range);
        std::mem::swap(&mut self.actions, &mut other.actions);
        std::mem::swap(&mut self.records, &mut other.records);
        std::mem::swap(&mut self.accumulations, &mut other.accumulations);
        std::mem::swap(&mut self.reqs.reqs, &mut other.reqs);

        // 重新设定统计间隔为当前开始
        self.time_range = PerfTimeRange::now();

        other
    }
}

impl PerfItemMerge<PerfIsolateImpl> for PerfIsolateImpl {
    fn merge(&mut self, mut other: PerfIsolateImpl) {
        assert_eq!(self.id, other.id);

        self.actions.append(&mut other.actions);
        self.records.merge(other.records);
        self.accumulations.merge(other.accumulations);
        self.reqs.merge(other.reqs);
    }
}

#[derive(Clone)]
pub struct PerfIsolate(Arc<Mutex<PerfIsolateImpl>>);

impl PerfIsolate {
    pub fn new(id: &str) -> Self {
        Self(Arc::new(Mutex::new(PerfIsolateImpl::new(id))))
    }

    pub fn begin_request(&self, id: &str, key: &str) {
        self.0.lock().unwrap().begin_request(id, key)
    }

    pub fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        self.0.lock().unwrap().end_request(id, key, err, bytes)
    }

    pub fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        self.0.lock().unwrap().acc(id, err, size)
    }

    pub fn action(
        &self,
        id: &str,
        err: BuckyErrorCode,
        name: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.0.lock().unwrap().action(id, err, name, value)
    }

    pub fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        self.0.lock().unwrap().record(id, total, total_size)
    }

    // 取走数据并置空
    pub(crate) fn take_data(&self) -> PerfIsolateEntity {
        self.0.lock().unwrap().take_data()
    }

    pub fn get_id(&self) -> String {
        self.0.lock().unwrap().id.clone()
    }
}
