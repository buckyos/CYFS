use std::ops::Sub;
use std::time::{Duration, Instant};

pub struct Timer {
    last_wake_time: Instant,
    duration: Duration,
}

impl Timer {
    pub fn new(duration: u64) -> Self {
        Self {
            last_wake_time: Instant::now(),
            duration: Duration::from_millis(duration),
        }
    }

    pub fn reset(&mut self, duration: u64) {
        self.duration = Duration::from_millis(duration);
        self.last_wake_time = Instant::now();
    }

    pub async fn wait_next(&mut self) {
        let elapsed = Instant::now().duration_since(self.last_wake_time);
        if elapsed < self.duration {
            let _ = async_std::future::timeout(
                self.duration.sub(elapsed),
                std::future::pending::<()>(),
            )
            .await;
        }
        self.last_wake_time = Instant::now();
    }
}
