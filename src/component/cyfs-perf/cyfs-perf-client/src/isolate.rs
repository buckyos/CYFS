use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_perf_base::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;

// 用以辅助request的区间统计
struct PerfRequestHolder {
    pub pending_reqs: HashMap<String, u64>,
    pub reqs: HashMap<String, PerfRequestDesc>,
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
                let bucky_time = bucky_time_now();
                let js_time = bucky_time_to_js_time(bucky_time);
                v.insert(js_time);
            }
            Entry::Occupied(_o) => {
                unreachable!("perf request item already begin! id={}, key={}", id, key);
            }
        }
    }

    pub fn end(&mut self, id: &str, key: &str, result: BuckyResult<u64>) {
        let bucky_time = bucky_time_now();
        let js_time = bucky_time_to_js_time(bucky_time);
        let full_id = format!("{}_{}", id, key);
        match self.pending_reqs.remove(&full_id) {
            Some(tick) => {
                let during = if js_time > tick {
                    js_time - tick
                } else {
                    0
                };

                match self.reqs.entry(id.to_owned()) {
                    Entry::Vacant(v) => {
                        let mut req = PerfRequestDesc {
                            time: TimeResult { total: during, avg: 0, min: u64::MAX, max: u64::MIN },
                            speed: SpeedResult { avg: 0., min: f32::MAX, max: f32::MIN },
                            size: SizeResult { total: 0, avg: 0, min: u64::MAX, max: u64::MIN },
                            success: 0,
                            failed: 0,
                        };

                        if let Ok(v) = result {
                            req.size.total = v;
                            req.success = 1;
                        } else {
                            req.failed = 1;
                        }

                        v.insert(req);
                    }
                    Entry::Occupied(mut o) => {
                        let item = o.get_mut();
                        if let Ok(v) = result {
                            item.success += 1;
                            item.size.total += v;
                            if v < item.size.min {
                                item.size.min = v;
                            }
                            if v > item.size.max {
                                item.size.max = v;
                            }

                        } else {
                            item.failed += 1;
                        }
                        item.size.avg = item.size.total / (item.success + item.failed) as u64;

                        item.time.total += during;
                        item.time.avg = item.time.total / (item.success + item.failed) as u64;
                        if during < item.time.min {
                            item.time.min = during;
                        }
                        if during > item.time.max {
                            item.time.max = during;
                        }

                        // todo 流量统计item speed

                    }
                }
            }
            None => {
                unreachable!();
            }
        }
    }
}

impl PerfItemMerge<PerfRequestHolder> for PerfRequestHolder {
    fn merge(&mut self, other: Self) {
        self.reqs.merge(other.reqs)
    }
}

struct PerfIsolateInner {
    id: String,

    actions: Vec<PerfActionDesc>,

    records: HashMap<String, PerfRecordDesc>,

    accumulations: HashMap<String, PerfAccumulationDesc>,

    reqs: PerfRequestHolder,
}

pub type PerfIsolateEntity = PerfIsolateInner;

impl PerfIsolateInner {
    pub fn new(id: &str) -> PerfIsolateInner {
        Self {
            id: id.to_owned(),
            actions: vec![],
            records: HashMap::new(),
            accumulations: HashMap::new(),
            reqs: PerfRequestHolder::new(),
        }
    }

    pub fn begin_request(&mut self, id: &str, key: &str) {
        self.reqs.begin(id, key);
    }

    pub fn end_request(&mut self, id: &str, key: &str, result: BuckyResult<u64>) {
        self.reqs.end(id, key, result);
    }

    pub fn acc(&mut self, id: &str, result: BuckyResult<u64>) {
        match self.accumulations.entry(id.to_owned()) {
            Entry::Vacant(v) => {
                let mut acc = PerfAccumulationDesc {
                    size: SizeResult { total: 0, avg: 0, min: u64::MAX, max: u64::MIN },
                    success: 0,
                    failed: 0,
                };
                if let Ok(v) = result {
                    acc.size.total = v;
                    acc.success = 1;
                } else {
                    acc.failed = 1;
                }

                v.insert(acc);
            }
            Entry::Occupied(mut o) => {
                let acc = o.get_mut();
                if let Ok(v) = result {
                    acc.success += 1;
                    acc.size.total += v;
                    if v < acc.size.min {
                        acc.size.min = v;
                    }
                    if v > acc.size.max {
                        acc.size.max = v;
                    }
                } else {
                    acc.failed += 1;
                }

                acc.size.avg =  acc.size.total / (acc.failed + acc.success)  as u64;

            }
        }
    }

    pub fn action(
        &mut self,
        id: &str,
        result: BuckyResult<(String, String)>,
    ) {
        let mut action = PerfActionDesc {
            err: BuckyErrorCode::Ok,
            key: "".into(),
            value: "".into(),
        };
        if let Ok((k, v)) = result {
            action.key = k;
            action.value = v;
        }

        self.actions.push(action);
    }

    pub fn record(&mut self, id: &str, total: u64, total_size: Option<u64>) {
        let record = PerfRecordDesc {
            total,
            total_size,
        };

        self.records.insert(id.to_owned(), record);
    }

    // 取走所有已有的统计项
    pub fn take_data(&mut self) -> PerfIsolateEntity {
        let mut other = PerfIsolateEntity::new(&self.id);

        std::mem::swap(&mut self.actions, &mut other.actions);
        std::mem::swap(&mut self.records, &mut other.records);
        std::mem::swap(&mut self.accumulations, &mut other.accumulations);
        std::mem::swap(&mut self.reqs, &mut other.reqs);

        other
    }
}

impl PerfItemMerge<PerfIsolateInner> for PerfIsolateInner {
    fn merge(&mut self, mut other: PerfIsolateInner) {
        assert_eq!(self.id, other.id);

        self.actions.append(&mut other.actions);
        self.records.merge(other.records);
        self.accumulations.merge(other.accumulations);
        self.reqs.merge(other.reqs);
    }
}

#[derive(Clone)]
pub struct PerfIsolate(Arc<Mutex<PerfIsolateInner>>);

impl PerfIsolate {
    pub fn new(id: &str) -> Self {
        Self(Arc::new(Mutex::new(PerfIsolateInner::new(id))))
    }

    // 开启一个request
    pub fn begin_request(&self, id: &str, key: &str) {
        self.0.lock().unwrap().begin_request(id, key)
    }
    // 统计一个操作的耗时, 流量统计
    pub fn end_request(&self, id: &str, key: &str, result: BuckyResult<u64>) {
        self.0.lock().unwrap().end_request(id, key, result)
    }

    pub fn acc(&self, id: &str, result: BuckyResult<u64>) {
        self.0.lock().unwrap().acc(id, result)
    }

    pub fn action(
        &self,
        id: &str,
        result: BuckyResult<(String, String)>,
    ) {
        self.0.lock().unwrap().action(id, result)
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
