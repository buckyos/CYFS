
use std::time::Duration;

pub mod types;
pub mod client;
pub mod service;

#[derive(Copy, Clone)]
pub struct Config {
    pub ping_interval_init: Duration,
    pub ping_interval: Duration,
    pub offline: Duration,

    pub call_interval: Duration,
    pub call_timeout: Duration,
}

impl std::default::Default for Config {
    fn default() -> Self {
        Self {
            ping_interval_init: Duration::from_millis(500),
            ping_interval: Duration::from_millis(25000),
            offline: Duration::from_millis(300000),
            call_interval: Duration::from_millis(200),
            call_timeout: Duration::from_millis(3000),
        }
    }
}
