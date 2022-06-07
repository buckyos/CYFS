use std::{
    time::Duration,
    sync::{Arc, atomic::{AtomicBool, AtomicPtr, Ordering}}, 
};
use cyfs_base::*;

pub trait PerfDataAbstract: std::fmt::Display + Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    fn clone_as_perfdata(&self) -> Box<dyn PerfDataAbstract>;

    fn byte_total(&self) -> u64;
    fn bandwidth(&self) -> u64;
}

#[derive(Clone)]
pub struct PerfData {
    ms_start_timestamp: u64,
    ms_last_timestamp: u64,
    ms_runtime: u64,

    pkt_packet_total: u64,    // total number of sent data packets
    byte_total: u64,
    mbps_max_rate: u64,
    mbps_avg_rate: u64,

    progress: usize,          // reserved for future
}

impl PerfData {
    fn new(ms_start_timestamp: u64,
           ms_last_timestamp: u64,
           pkt_packet_total: u64,    // total number of sent data packets
           byte_total: u64,
           mbps_max_rate: u64,
           mbps_avg_rate: u64) -> Self {
        Self {
            ms_start_timestamp: ms_start_timestamp,
            ms_last_timestamp: ms_last_timestamp,
            ms_runtime: 0u64,
            pkt_packet_total: pkt_packet_total,    // total number of sent data packets
            byte_total: byte_total,
            mbps_max_rate: mbps_max_rate,
            mbps_avg_rate: mbps_avg_rate,
            progress: 0usize,          // reserved for future        
        }
    }

    fn reset(&mut self,
             ms_start_timestamp: u64,
             ms_last_timestamp: u64) {
        self.ms_start_timestamp = ms_start_timestamp;
        self.ms_last_timestamp = ms_last_timestamp;
        self.ms_runtime = 0u64;
        self.pkt_packet_total = 0u64;
        self.byte_total = 0u64;
        self.mbps_max_rate = 0u64;
        self.mbps_avg_rate = 0u64;
        self.progress = 0usize;
    }

    #[inline]
    pub fn ms_start_timestamp(&self) -> u64 {
        self.ms_start_timestamp
    }
    #[inline]
    pub fn ms_last_timestamp(&self) -> u64 {
        self.ms_last_timestamp
    }
    #[inline]
    pub fn pkt_packet_total(&self) -> u64 {
        self.pkt_packet_total
    }
    #[inline]
    pub fn byte_total(&self) -> u64 {
        self.byte_total
    }
    #[inline]
    pub fn mbps_max_rate(&self) -> u64 {
        self.mbps_max_rate
    }
    #[inline]
    pub fn mbps_avg_rate(&self) -> u64 {
        self.mbps_avg_rate
    }

    #[inline]
    pub fn set_progress(&mut self, progress: usize) {
        self.progress = progress;
    }
    #[inline]
    pub fn progress(&self) -> usize {
        self.progress
    }
}

impl PerfDataAbstract for PerfData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_as_perfdata(&self) -> Box<dyn PerfDataAbstract> {
        Box::new(self.clone())
    }

    fn byte_total(&self) -> u64 {
        self.byte_total()
    }

    fn bandwidth(&self) -> u64 {
        self.mbps_avg_rate()
    }

}

impl std::default::Default for PerfData {
    fn default() -> Self {
        Self {
            ms_start_timestamp: 0u64,
            ms_last_timestamp: 0u64,
            ms_runtime: 0u64,
            pkt_packet_total: 0u64,     // total number of sent data packets
            byte_total: 0u64,
            mbps_max_rate: 0u64,
            mbps_avg_rate: 0u64,
            progress: 0usize,           // reserved for future        
        }
    }
}

impl std::fmt::Display for PerfData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pkt_packet_total={}, byte_total={}, mbps_avg_rate={}, mbps_max_rate={}",
            self.pkt_packet_total,
            self.byte_total,
            self.mbps_avg_rate,
            self.mbps_max_rate)
    }
}

pub trait StatisticTask: std::fmt::Display + Send + Sync {
    fn reset(&self);

    fn stat(&self) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        Err(BuckyError::new(BuckyErrorCode::Ignored, ""))
    }

    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>>;
}

pub type StatisticTaskPtr = Arc<dyn StatisticTask>;

#[derive(Clone)]
pub struct DynamicStatisticTask(StatisticTaskPtr);

impl DynamicStatisticTask {
    pub fn new<S: 'static + StatisticTask>(task: S) -> Self {
        Self(
            Arc::new(task)
        )
    }

    pub fn default() -> Self {
        DynamicStatisticTask::new(AtomicStatisticTask::default())
    }

    pub fn ptr(&self) -> StatisticTaskPtr {
        self.0.clone()
    }
}

impl From<StatisticTaskPtr> for DynamicStatisticTask {
    fn from(task: StatisticTaskPtr) -> Self {
        Self (
            Arc::from(task)
        )
    }

}

impl StatisticTask for DynamicStatisticTask {
    fn reset(&self) {
        self.0.reset()
    }

    fn stat(&self) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        self.0.stat()
    }

    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        self.0.on_stat(size)
    }

}

impl std::fmt::Display for DynamicStatisticTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }

}

pub struct AtomicStatisticTask {
    lock_flag: AtomicBool,
    data: AtomicPtr<PerfData>,
}

impl AtomicStatisticTask {
    pub fn default() -> Self {
        let now = bucky_time_now();

        let data = Box::new(PerfData::new(now.clone(),
                                                        now.clone(),
                                                        0u64,
                                                        0u64,
                                                        0u64,
                                                        0u64));
        // let data_clone = data;
        let mut_data = Box::into_raw(data);

        Self {
            lock_flag: AtomicBool::new(false),
            data: AtomicPtr::new(mut_data),
        }
    }

    fn lock(&self) {
        loop {
            if let Ok(_) = self.lock_flag.compare_exchange(false, 
                                                           true, 
                                                           Ordering::Acquire, 
                                                           Ordering::Acquire) {
                break;
            }
        }
    }

    fn unlock(&self) {
        self.lock_flag.store(false, Ordering::Release);
    }
}

impl std::fmt::Display for AtomicStatisticTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe {
            let data = self.data.load(Ordering::Acquire);
            let data = &*data;

            write!(f, "last-timestamp={}, diff-timestamp={}, packet-total={}, byte_total={}, rate=[max={}, cur={}]", 
                   data.ms_last_timestamp(),
                   data.ms_last_timestamp() - data.ms_start_timestamp(),
                   data.pkt_packet_total(),
                   data.byte_total(),
                   data.mbps_max_rate(), data.mbps_avg_rate())
        }
    }

}

impl StatisticTask for AtomicStatisticTask {
    fn reset(&self) {
        self.lock();

        let now = bucky_time_now();

        unsafe {
            let data = self.data.load(Ordering::Acquire);
            let data = &mut *data;

            data.reset(now.clone(), now.clone());
        }

        self.unlock();
    }

    fn stat(&self) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        unsafe {
            self.lock();

            let data = self.data.load(Ordering::Acquire);
            let data_cp = (*data).clone_as_perfdata();

            self.unlock();

            Ok(data_cp)
        }
    }

    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        self.lock();

        let now = bucky_time_now();

        unsafe {
            let data = self.data.load(Ordering::Acquire);
            let data = &mut *data;

            data.pkt_packet_total += 1;
            data.ms_last_timestamp = now.clone();

            if now > data.ms_start_timestamp {
                let time_diff = Duration::from_micros(now.clone() - data.ms_start_timestamp).as_secs_f32();
                let byte_total = 
                    {
                        data.byte_total = data.byte_total + size;
                        data.byte_total
                    };

                let avg_rate = (byte_total as f32/ time_diff) as u64;
                data.mbps_avg_rate = avg_rate.clone();

                if avg_rate > data.mbps_max_rate {
                    data.mbps_max_rate = avg_rate.clone();
                }

            }

            // let data_cp = Box::new(data.clone());
            let data_cp = data.clone_as_perfdata();

            self.unlock();

            Ok(data_cp)
        }
    }

}
