use std::time::{Instant, Duration};

#[derive(Debug)]
pub struct Pacer {
    capacity: usize,
    used: usize,
    rate: u64,
    last_update: Instant,
    next_time: Instant,
    mss: usize,
    last_packet_size: Option<usize>,
    tick: Duration,
}

impl Pacer {
    pub fn new(capacity: usize, mss: usize) -> Self {
        let capacity = capacity / mss * mss;

        Pacer {
            capacity,
            used: 0,
            rate: 0,
            last_update: Instant::now(),
            next_time: Instant::now(),
            mss,
            last_packet_size: None,
            tick: Duration::ZERO,
        }
    }

    pub fn update(&mut self, rate: u64) {
        if self.rate != rate {
            self.rate = rate;
        }
    }

    pub fn reset(&mut self, now: Instant) {
        self.used = 0;
        self.last_update = now;
        self.next_time = self.next_time.max(now);
        self.last_packet_size = None;
        self.tick = Duration::ZERO;
    }

    pub fn send(&mut self, packet_size: usize, now: Instant) -> Option<Instant> {
        if self.rate == 0 {
            self.reset(now);

            return None;
        }

        if !self.tick.is_zero() {
            self.next_time = self.next_time.max(now) + self.tick;
            self.tick = Duration::ZERO;
        }

        let interval = Duration::from_secs_f64(self.capacity as f64 / self.rate as f64);

        let elapsed = now.saturating_duration_since(self.last_update);

        if elapsed > interval {
            self.reset(now);
        }

        self.used += packet_size;

        let same_size = if let Some(last_packet_size) = self.last_packet_size {
            last_packet_size == packet_size
        } else {
            true
        };

        self.last_packet_size = Some(packet_size);

        if self.used >= self.capacity || !same_size {
            self.tick = Duration::from_secs_f64(self.used as f64 / self.rate as f64);
            self.used = 0;
            self.last_update = now;
            self.last_packet_size = None;
        };

        if self.next_time <= now {
            None
        } else {
            Some(self.next_time)
        }
    }
}
