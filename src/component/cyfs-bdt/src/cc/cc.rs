use std::{
    time::Duration
};
use crate::types::*;
use super::{
    cc_impl::CcImpl, 
    ledbat::{self, Ledbat},
    bbr::{self, Bbr},
};


#[derive(Debug)]
struct EstimateRtt {
    srtt: i64, // smoothed round trip time << 3
    mdev: i64, //medium deviation
    mdev_max: i64, //maximal m_dev for the last rtt period
    rttvar: i64, //smoothed m_dev_max
    // rtt_seq: i64, //sequence number to update rtt_var
    min_rtt: i64,
}

impl EstimateRtt {
    fn new() -> Self {
        Self {
            srtt: 0,
            mdev: Duration::from_secs(1).as_micros() as i64,
            mdev_max: 0,
            rttvar: 0,
            // rtt_seq: 0,
            min_rtt: 0,
        }
    }

    fn update(&mut self, config: &Config, rtt: Duration) -> (Duration/*srtt*/, Duration/*rto*/) {
        let mut rtt = rtt.as_micros() as i64;
        if rtt == 0 {
            rtt = 1;
        }

        if self.min_rtt == 0 ||
            self.min_rtt > rtt {
            self.min_rtt = rtt;
        }

        if self.srtt != 0 {
            rtt -= self.srtt / 8;
            self.srtt += rtt;
            if rtt < 0 {
                rtt = 0 - rtt;
                rtt -= self.mdev / 4;
                if rtt > 0 {
                    rtt >>= 3;
                }
            } else {
                rtt -= self.mdev / 4;
            }

            self.mdev = self.mdev + rtt;

            if self.mdev > self.mdev_max {
                self.mdev_max = self.mdev;
                if self.mdev_max > self.rttvar {
                    self.rttvar = self.mdev_max;
                }
            }
            if self.mdev < self.mdev_max {
                self.rttvar -= (self.rttvar - self.mdev_max) / 4;
            }
            self.mdev_max = config.min_rto.as_micros() as i64;
        } else {
            self.srtt = rtt * 8;
            self.mdev = rtt * 2;
            self.rttvar = std::cmp::max(self.mdev, config.min_rto.as_micros() as i64);
            self.mdev_max = self.rttvar;
        }
        let srtt = (self.srtt / 8) as u64;
        let rto = (self.srtt / 8 + self.rttvar) as u64;
        (Duration::from_micros(srtt), Duration::from_micros(rto))
    }
}

#[derive(Clone)]
pub enum ImplConfig {
    Ledbat(ledbat::Config),
    BBR(bbr::Config),
}

#[derive(Clone)]
pub struct Config {
    pub init_rto: Duration, 
    pub min_rto: Duration, 
    pub cc_impl: ImplConfig
}

pub struct CongestionControl {
    rtt: Duration, 
    rto: Duration, 
    config: Config, 
    est_rtt: EstimateRtt, 
    cc: Box<dyn CcImpl>, 
}

impl CongestionControl {
    pub fn new(mss: usize, config: &Config) -> Self {
        Self {
            rtt: Duration::from_secs(0), 
            rto: config.init_rto, 
            est_rtt: EstimateRtt::new(), 
            cc: match &config.cc_impl { 
                ImplConfig::Ledbat(config) => {
                    Box::new(Ledbat::new(mss, config))
                },
                ImplConfig::BBR(config) => {
                    Box::new(Bbr::new(mss, config))
                }
            },
            config: config.clone()
        }
    }
    
    pub fn on_sent(&mut self, now: Timestamp, bytes: u64, last_packet_number: u64) {
        self.cc.on_sent(now, bytes, last_packet_number);
    }

    pub fn cwnd(&self) -> u64 {
        self.cc.cwnd()
    }

    pub fn rto(&self) -> Duration {
        self.rto
    }

    pub fn rtt(&self) -> Duration {
        self.rtt
    }

    pub fn on_estimate(&mut self, est_rtt: Duration, est_delay: Duration, app_limited: bool) {
        let (rtt, rto) = self.est_rtt.update(&self.config, est_rtt);
        self.rto = rto;
        self.rtt = rtt;
        self.cc.on_estimate(Duration::from_micros(self.est_rtt.min_rtt as u64), rto, est_delay, app_limited);
    }

    pub fn on_ack(&mut self,
        flight: u64, 
        ack: u64,
        largest_packet_num_acked: Option<u64>, 
        sent_time: Timestamp,
        app_limited: bool) {
        self.cc.on_ack(flight, ack, largest_packet_num_acked, sent_time, app_limited)
    }

    pub fn on_loss(&mut self, lost: u64) {
        self.cc.on_loss(lost)
    }

    pub fn on_no_resp(&mut self, lost: u64) {
        let rto = self.cc.on_no_resp(self.rto, lost);
        self.rto = rto;
    }

    pub fn on_time_escape(&mut self, now: Timestamp) {
        self.cc.on_time_escape(now)
    }

    pub fn rate(&self) -> u64 {
        self.cc.rate()
    }
}