use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub struct Timer {
    sleep: Option<Pin<Box<dyn Future<Output = ()>>>>,
    duration: u64,
}

impl Timer {
    pub fn new(duration: u64) -> Self {
        let sleep = Box::pin(async {
            async_std::future::timeout(
                Duration::from_millis(duration),
                std::future::pending::<()>(),
            )
            .await;
        });
        Self {
            sleep: Some(sleep),
            duration,
        }
    }

    pub fn reset(&mut self, duration: u64) {
        let sleep = Box::pin(async {
            async_std::future::timeout(
                Duration::from_millis(duration),
                std::future::pending::<()>(),
            )
            .await;
        });
        self.duration = duration;
        self.sleep = Some(sleep);
    }

    pub async fn wait_next(&mut self) {
        self.sleep.take().unwrap().await;
        self.reset(self.duration);
    }
}
