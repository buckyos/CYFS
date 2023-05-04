use cyfs_base::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;


#[derive(Clone)]
pub struct TaskAbortHandler {
    aborted: Arc<AtomicBool>
}

impl TaskAbortHandler {
    pub fn new() -> Self {
        Self {
            aborted: Arc::new(AtomicBool::new(false))
        }
    }

    pub fn abort(&self) {
        self.aborted.store(true, Ordering::SeqCst);
    }

    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }

    pub fn check_aborted(&self) -> BuckyResult<()> {
        match self.is_aborted() {
            true => {
                let msg = format!("task already been aborted!");
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Aborted, msg))
            }
            false => {
                Ok(())
            }
        }
    }
}