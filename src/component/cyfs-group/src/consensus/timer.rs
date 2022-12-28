use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub struct Timer {
    sleep: Pin<Box<dyn Future<Output = ()>>>,
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
        Self { sleep }
    }

    pub fn reset(&mut self, duration: u64) {
        self.sleep = Box::pin(async {
            async_std::future::timeout(
                Duration::from_millis(duration),
                std::future::pending::<()>(),
            )
            .await;
        });
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.sleep.as_mut().poll(cx)
    }
}
