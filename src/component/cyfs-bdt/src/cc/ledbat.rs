use std::{
    time::{Duration}, 
    collections::LinkedList, 
};
use cyfs_base::*;
use crate::types::*;
use super::cc_impl::CcImpl;

#[derive(Clone)]
pub struct Config {
    pub target_delay: Duration, 
    pub min_cwnd: u64, 
    pub max_cwnd_inc: u64, 
    pub cwnd_gain: u64, 
    pub history_count: u64, 
    pub history_roll_interval: Duration
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target_delay: Duration::from_millis(100),
            min_cwnd: 2,
            max_cwnd_inc: 8,
            cwnd_gain: 1,
            history_count: 10,
            history_roll_interval: Duration::from_secs(60),
        }
    }
}

struct EstimateDelay {
    last_roll: Timestamp, 
    base_delay: LinkedList<i64>, 
    current_delay: LinkedList<i64>
}

impl EstimateDelay {
    fn new(config: &Config) -> Self {
        let mut base_delay = LinkedList::new();
        for _ in 0..config.history_count {
            base_delay.push_back(i64::MAX);
        }
        let mut current_delay = LinkedList::new();
        for _ in 0..config.history_count {
            current_delay.push_back(i64::MAX);
        }

        Self {
            last_roll: bucky_time_now() - config.history_roll_interval.as_micros() as u64, 
            base_delay, 
            current_delay, 
        }
    }

    fn current_delay(&self) -> i64 {
        let mut delay = i64::MAX;
        for d in &self.current_delay {
            delay = std::cmp::min(*d, delay);
        }
        delay
    }

    fn base_delay(&self) -> i64 {
        let mut delay = i64::MAX;
            for d in &self.base_delay {
                delay = std::cmp::min(*d, delay);
            }
            delay
    }

    fn update(&mut self, delay: i64) {
        let tail = self.base_delay.back_mut().unwrap();
        *tail = std::cmp::min(*tail, delay);  

        self.current_delay.pop_front();
        self.current_delay.push_back(delay);
    }

    fn check_roll(&mut self, config: &Config, now: Timestamp) {
        if now > self.last_roll && Duration::from_micros(now - self.last_roll) > config.history_roll_interval {
            self.last_roll = now;
            self.base_delay.pop_front();
            self.base_delay.push_back(i64::MAX);

            let base_delay = {
                let mut delay = i64::MAX;
                for d in &self.base_delay {
                    delay = std::cmp::min(*d, delay);
                }
                delay
            };
            if base_delay == i64::MAX {
                for d in &mut self.current_delay {
                    *d = i64::MAX;
                }
            }
        }
    }
}


pub(super) struct Ledbat {
    mss: usize, 
    est_delay: EstimateDelay,
    config: Config, 
    cwnd: u64
}

impl Ledbat {
    pub fn new(mss: usize, config: &Config) -> Self {
        let mut config = config.clone();
        config.min_cwnd = config.min_cwnd * mss as u64;
        config.cwnd_gain = config.cwnd_gain * mss as u64;
        config.max_cwnd_inc = config.max_cwnd_inc * mss as u64;
        Self {
            cwnd: config.min_cwnd, 
            mss, 
            est_delay: EstimateDelay::new(&config), 
            config
        }
    }
}


impl CcImpl for Ledbat {
    fn on_sent(&mut self, _: Timestamp, _: u64, _: u64) {
    }

    fn cwnd(&self) -> u64 {
        self.cwnd
    }

    fn on_estimate(&mut self, _rtt: Duration, _rto: Duration, delay: Duration, _app_limited: bool) {
        self.est_delay.update(delay.as_micros() as i64);
    }

    fn on_ack(
        &mut self, 
        _flight: u64, 
        ack: u64, 
        _largest_packet_num_acked: Option<u64>, 
        _sent_time: Timestamp,
        _app_limited: bool
    ) {
        let cwnd = self.cwnd();
        let cur_delay = self.est_delay.current_delay();
        let base_delay = self.est_delay.base_delay();
        let queuing_delay = cur_delay - base_delay;
        let delay_factor = (self.config.target_delay.as_micros() as i64 - queuing_delay) as f64 / self.config.target_delay.as_micros() as f64;
        let cwnd_factor = std::cmp::min(ack, cwnd) as f64 / std::cmp::max(ack, cwnd) as f64;
        let scaled_gain = (self.config.max_cwnd_inc as f64 * cwnd_factor * delay_factor) as i64;
        let new_cwnd = (cwnd as i64 + scaled_gain) as u64;
        // trace!("ledbat cur_delay:{} base_delay:{} queuing_delay:{} delay_factor:{} cwnd_factor:{} scaled_gain:{}", cur_delay, base_delay, queuing_delay, delay_factor, cwnd_factor, scaled_gain);
        // let allowed_max = (flight + newly_acked + config.wnd_gain as u64 * self.mss as u64) as i64;
        // new_wnd = std::cmp::min(new_wnd, allowed_max);
        self.cwnd = new_cwnd.max(self.config.min_cwnd);
    }

    
    fn on_loss(&mut self, _lost: u64) {
        self.cwnd = (self.cwnd / 2).max(self.config.min_cwnd)
    }

    fn on_no_resp(&mut self, rto: Duration, _lost: u64) -> Duration {
        self.cwnd = self.config.min_cwnd;
        rto * 2
    }

    fn on_time_escape(&mut self, now: Timestamp) {
        self.est_delay.check_roll(&self.config, now);   
    }

    fn rate(&self) -> u64 {
        0
    }
}

