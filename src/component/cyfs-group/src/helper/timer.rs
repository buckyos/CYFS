use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct Timer {
    sleep: Option<Pin<Box<dyn Send + Sync + Future<Output = ()>>>>,
    duration: u64,
}

impl Timer {
    pub fn new(duration: u64) -> Self {
        let sleep = Box::pin(async move {
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
        let sleep = Box::pin(async move {
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
        let sleep = self.sleep.take().unwrap();
        self.reset(self.duration);
        sleep.await;
    }
}
