use std::{sync::{Arc, Mutex}, collections::HashMap};

use cyfs_base::BuckyErrorCode;

use super::*;

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
    results: Vec<u32>, //us
    errs: Vec<BuckyErrorCode>,
    remote_offline: Vec<BuckyErrorCode>,
    remote_not_exists: Vec<BuckyErrorCode>,
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
            errs: vec![],
            remote_offline: vec![],
            remote_not_exists: vec![],
            qps: 0,
        })))
    }

    pub fn add_resp_time(&self, resp_time: u64) {
        let mut result = self.0.lock().unwrap();
        result.results.push(resp_time as u32);
    }

    pub fn add_error(&self, exception: ExceptionType, err_code: BuckyErrorCode) {
        let mut result = self.0.lock().unwrap();
        match exception {
            ExceptionType::RemoteOffline => result.remote_offline.push(err_code),
            ExceptionType::RemoteNotExists => result.remote_not_exists.push(err_code),
            _ => result.errs.push(err_code)
        }
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

        if result.time_total > 0 {
            result.qps = (result.success * 1000 * 1000) / result.time_total;
        }
    }

    pub fn show(&self) {
        let result = self.0.lock().unwrap();

        let mut hm = HashMap::new();
        for (_, bec) in result.remote_not_exists.iter().enumerate() {
            let count = hm.entry(bec.to_string()).or_insert(0);
            *count += 1;
        }
        for (_, bec) in result.remote_offline.iter().enumerate() {
            let count = hm.entry(bec.to_string()).or_insert(0);
            *count += 1;
        }
        for (_, bec) in result.errs.iter().enumerate() {
            let count = hm.entry(bec.to_string()).or_insert(0);
            *count += 1;
        }

        println!("qps={}", result.qps);
        println!("time_mean={:.2} ms", result.time_mean as f64/1000.0);
        println!("time_min={:.2} ms", result.time_min as f64/1000.0);
        println!("time_max={:.2} ms", result.time_max as f64/1000.0);
        println!("time_total={:.2} ms", result.time_total as f64/1000.0);
        println!("count={}", result.count);
        println!("success={}", result.success);
        println!("exception:");
        for (err, count) in hm.iter() {
            println!("  {}={}", err, count);
        }
    }
}