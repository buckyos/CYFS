use std::sync::{Arc, Mutex};

pub struct ErrorType {
    code: u32,
    short_msg: String,
}

pub struct SnBenchResultImpl {
    start: u64,
    end: u64,
    count: u64,
    success: u64,
    time_total: u64,
    time_min: u64,
    time_max: u64,
    time_mean: u64,
    results: Vec<i16>,
    err_types: Vec<ErrorType>,
    qps: u64,
}

#[derive(Clone)]
pub struct SnBenchResult(Arc<Mutex<SnBenchResultImpl>>);

impl SnBenchResult {
    pub fn default() -> Self {
        SnBenchResult(Arc::new(Mutex::new(SnBenchResultImpl {
            start: 0,
            end: 0,
            count: 0,
            success: 0,
            time_total: 0,
            time_min: 0,
            time_max: 0,
            time_mean: 0,
            results: Vec::new(),
            err_types: vec![],
            qps: 0,
        })))
    }

    pub fn add_resp_time(&self, resp_time: i16) {
        let mut result = self.0.lock().unwrap();
        result.results.push(resp_time);
    }

    pub fn add_error(&self, err_type: ErrorType) {
        let mut result = self.0.lock().unwrap();
        result.err_types.push(err_type);
    }

    pub fn stat(&self, start: u64, end: u64) {
        let mut result = self.0.lock().unwrap();
        let count = result.results.len() as u64;
        let mut time_total = 0;
        let mut success = 0;
        let mut time_max = 0;
        let mut time_min = 0;
        for (_, resp_time) in result.results.iter().enumerate() {
            if *resp_time > 0 {
                let resp_time = *resp_time as u64;

                success += 1;
                time_total += resp_time;
                if time_max < resp_time {
                    time_max = resp_time;
                }
                if time_min > resp_time || time_min == 0 {
                    time_min = resp_time;
                }
            }
        }

        if success > 0 {
            result.time_mean = time_total / success;
        }
        result.success = success;
        result.time_total = time_total;
        result.time_max = time_max;
        result.time_min = time_min;
        result.count = count;
        result.start = start;
        result.end = end;

        let cost = (end - start)/1000/1000;
        if cost > 0 {
            result.qps = success / cost;
        }
    }

    pub fn show(&self) {
        let result = self.0.lock().unwrap();

        println!("qps={}", result.qps);
        println!("time_mean={}", result.time_mean);
        println!("time_total={}", result.time_total);
        println!("time_min={}", result.time_min);
        println!("time_max{}", result.time_max);
        println!("count={}", result.count);
        println!("success={}", result.success);
    }
}