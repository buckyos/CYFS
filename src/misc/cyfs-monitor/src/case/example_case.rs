use crate::def::*;
use cyfs_base::*;

pub struct ExampleCaseMonitor {
    return_err: bool
}

impl ExampleCaseMonitor {
    pub fn new(return_err: bool) -> Self {
        Self {
            return_err
        }
    }
}

#[async_trait::async_trait]
impl MonitorRunner for ExampleCaseMonitor {
    fn name(&self) -> &str {
        "example_case"
    }

    async fn run_once(&self, _once: bool) -> BuckyResult<()> {
        if self.return_err {
            Err(BuckyError::new(BuckyErrorCode::Failed, "test return error"))
        } else {
            Ok(())
        }
    }
}
