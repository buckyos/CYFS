use cyfs_base::*;

#[async_trait::async_trait]
pub trait MonitorRunner {
    fn name(&self) -> &str;
    async fn run_once(&self, once: bool) -> BuckyResult<()>;
}

#[derive(Debug, Clone)]
pub struct MonitorErrorInfo {
    pub service: String,
    pub case: String,
    pub error: BuckyError,
    pub at_all: bool
}

// 上报一个错误
#[async_trait::async_trait]
pub trait BugReporter {
    async fn report_error(&self, info: &MonitorErrorInfo) -> BuckyResult<()>;
}
