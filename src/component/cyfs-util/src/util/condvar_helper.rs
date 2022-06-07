use std::sync::Arc;
use async_std::sync::{Condvar, Mutex};

pub struct AsyncCondvar {
    mutex: Mutex<bool>,
    condvar: Condvar,
}
pub type AsyncCondvarRef = Arc<AsyncCondvar>;

impl AsyncCondvar {
    pub fn new() -> AsyncCondvarRef {
        AsyncCondvarRef::new(Self {
            mutex: Mutex::new(false),
            condvar: Condvar::new()
        })
    }

    pub fn notify(&self) {
        self.condvar.notify_one();
    }

    pub async fn wait(&self) {
        let mutex = self.mutex.lock().await;
        self.condvar.wait(mutex).await;
    }
}
