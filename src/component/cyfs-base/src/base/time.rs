use std::time::{Duration, SystemTime, UNIX_EPOCH};

static _TIME_TTO_MICROSECONDS_OFFSET: u64 = 11644473600_u64 * 1000 * 1000;

pub const MIN_BUCKY_TIME: u64 = 11644473600_u64 * 1000 * 1000;

pub fn system_time_to_bucky_time(time: &SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).unwrap().as_micros() as u64 + _TIME_TTO_MICROSECONDS_OFFSET
}

pub fn bucky_time_to_system_time(time: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_micros(bucky_time_to_unix_time(time))
}

// 转换为UNIX_EPOCH开始的微秒数
pub fn bucky_time_to_unix_time(time: u64) -> u64 {
    if time <= _TIME_TTO_MICROSECONDS_OFFSET {
        return 0;
    }

    time - _TIME_TTO_MICROSECONDS_OFFSET
}

// in micro seconds of unix epoch
pub fn unix_time_to_bucky_time(time: u64) -> u64 {
    time + _TIME_TTO_MICROSECONDS_OFFSET
}

pub fn bucky_time_now() -> u64 {
    system_time_to_bucky_time(&SystemTime::now())
}

// js time以毫秒为单位
pub fn js_time_to_bucky_time(time: u64) -> u64 {
    time * 1000 + _TIME_TTO_MICROSECONDS_OFFSET
}

pub fn bucky_time_to_js_time(time: u64) -> u64 {
    if time <= _TIME_TTO_MICROSECONDS_OFFSET {
        return 0;
    }
    ((time - _TIME_TTO_MICROSECONDS_OFFSET) as f64 / 1000f64) as u64
}

#[test]
fn test() {
    let bucky_time = 13248879111201108u64;
    let js_time = bucky_time_to_js_time(bucky_time);
    let bucky_time2 = js_time_to_bucky_time(js_time);

    println!("{} -> {} -> {}", bucky_time, js_time, bucky_time2);
    bucky_time_to_js_time(0);
    js_time_to_bucky_time(0);
}
