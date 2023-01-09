use cyfs_base::*;

#[derive(Debug, Clone)]
pub struct StatInfo {
    pub attachment: Vec<String>,
    pub context: String,
}

// 上报状态数据
#[async_trait::async_trait]
pub trait StatReporter {
    async fn report_stat(&self, info: &StatInfo) -> BuckyResult<()>;
}