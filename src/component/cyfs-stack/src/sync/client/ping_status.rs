use cyfs_base::{bucky_time_now, BuckyErrorCode};
use cyfs_lib::*;
use cyfs_debug::Mutex;

use std::sync::Arc;

#[derive(Clone)]
struct PingStatusInner {
    network: OODNetworkType,

    first_ping: u64,
    first_success_ping: u64,
    last_success_ping: u64,

    last_ping: u64,
    last_ping_result: BuckyErrorCode,

    ping_count: u32,
    ping_success_count: u64,

    // 当前连续失败的次数，成功后重置
    cont_fail_count: u64,

    ping_total_during: u64,
    ping_max_during: u64,
    ping_min_during: u64,
}

impl PingStatusInner {
    fn new() -> Self {
        Self {
            network: OODNetworkType::Unknown,

            first_ping: 0,
            first_success_ping: 0,
            last_success_ping: 0,

            last_ping: 0,
            last_ping_result: BuckyErrorCode::Ok,

            ping_count: 0,
            ping_success_count: 0,
            cont_fail_count: 0,

            ping_total_during: 0,
            ping_max_during: 0,
            ping_min_during: 0,
        }
    }

    pub fn on_ping_failed(&mut self, ping_result: BuckyErrorCode) {
        let now = bucky_time_now();
        self.on_ping(now, ping_result);

        self.cont_fail_count += 1;
    }

    pub fn on_ping_success(&mut self, network: OODNetworkType, ping_result: BuckyErrorCode, ping_during: u64) {
        let network_changed = if self.network != network {
            info!("ood network changed: {} -> {}", self.network, network);
            self.network = network;
            true
        } else {
            false
        };

        // 如果网络类型切换了，那么需要重新计算一些统计数据
        if network_changed {
            self.ping_count = 0;
            self.ping_success_count = 0;

            self.ping_total_during = 0;
            self.ping_max_during = 0;
            self.ping_min_during = 0;
        }

        let now = bucky_time_now();
        self.on_ping(now, ping_result);


        if self.first_success_ping == 0 {
            self.first_success_ping = now;
        }

        self.last_success_ping = now;
        self.ping_success_count += 1;
        self.cont_fail_count = 0;

        self.ping_total_during += ping_during;
        if ping_during > self.ping_max_during {
            self.ping_max_during = ping_during;
        }
        if self.ping_min_during == 0 || ping_during < self.ping_min_during {
            self.ping_min_during = ping_during;
        }
  
    }

    fn on_ping(&mut self, now: u64, ping_result: BuckyErrorCode,) {
        if self.first_ping == 0 {
            self.first_ping = now;
        }

        self.last_ping = now;
        self.last_ping_result = ping_result;
        self.ping_count += 1;
    }

    pub fn fill_ood_status(&self, status: &mut OODStatus) {
        let ping_avg_during = if self.ping_success_count > 0 {
            self.ping_total_during / self.ping_success_count
        } else {
            0u64
        };

    
        status.network= self.network.clone();

        status.first_ping= self.first_ping;
        status.first_success_ping= self.first_success_ping;
        status.last_success_ping= self.last_success_ping;

        status.last_ping= self.last_ping;
        status.last_ping_result= self.last_ping_result.into();

        status.ping_count= self.ping_count;
        status.ping_success_count= self.ping_success_count;

        status.cont_fail_count= self.cont_fail_count;

        status.ping_avg_during = ping_avg_during;
        status.ping_max_during = self.ping_max_during;
        status.ping_min_during = self.ping_min_during;
        
    }
}
/*
impl Into<OODStatus> for PingStatusInner {
    fn into(self) -> OODStatus {
        let ping_avg_during = if self.ping_success_count > 0 {
            self.ping_total_during / self.ping_success_count
        } else {
            0u64
        };

        OODStatus {
            network: self.network,

            first_ping: self.first_ping,
            first_success_ping: self.first_success_ping,
            last_success_ping: self.last_success_ping,

            last_ping: self.last_ping,
            last_ping_result: self.last_ping_result.into(),

            ping_count: self.ping_count,
            ping_success_count: self.ping_success_count,

            cont_fail_count: self.cont_fail_count,

            ping_avg_during,
            ping_max_during: self.ping_max_during,
            ping_min_during: self.ping_min_during,
        }
    }
}
*/
#[derive(Clone)]
pub struct PingStatus(Arc<Mutex<PingStatusInner>>);

impl PingStatus {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(PingStatusInner::new())))
    }

    pub fn on_ping_failed(&self, ping_result: BuckyErrorCode) {
        self.0.lock().unwrap().on_ping_failed(ping_result)
    }

    pub fn on_ping_success(&self, network: OODNetworkType, ping_result: BuckyErrorCode, ping_during: u64) {
        self.0.lock().unwrap().on_ping_success(network, ping_result, ping_during)
    }

    pub fn fill_ood_status(&self, status: &mut OODStatus) {
        self.0.lock().unwrap().fill_ood_status(status)
    }
}
