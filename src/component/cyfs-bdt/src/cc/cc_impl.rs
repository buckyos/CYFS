use std::time::{Duration};
use crate::{
    types::*, 
};

pub trait CcImpl: Send {
    fn cwnd(&self) -> u64;
    fn on_estimate(&mut self, rtt: Duration, rto: Duration, delay: Duration);
    fn on_ack(&mut self, flight: u64, ack: u64, largest_packet_num_acked: Option<u64>, sent_time: Timestamp);
    fn on_loss(&mut self, lost: u64);
    fn on_no_resp(&mut self, rto: Duration, lost: u64) -> Duration;
    fn on_time_escape(&mut self, now: Timestamp);
}
