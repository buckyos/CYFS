use std::sync::Arc;
use std::collections::HashMap;
use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode};
use crate::notify::*;

use crate::storage::{Storage, Period, MetaStat};

pub struct Client {
    storage: Arc<Box<dyn Storage + Send + Sync>>,
    deadline: u16,
    reporter: Notifier,
}

impl Client {
    pub(crate) fn new(url: &str, deadline: u16, storage: Arc<Box<dyn Storage + Send + Sync>>) -> Self {
        let reporter = Notifier::new(url);
        Self {
            storage,
            deadline,
            reporter,
        }
    }

    pub async fn run(&self) {
        // 概况
        if let Ok(ret) = self.get_desc().await {
            info!("{:?}", ret);
        }
        // 日/周/月表
        if let Ok(ret) = self.period_stat(Period::Daily).await {
            info!("{:?}", ret);
        }
        if let Ok(ret) = self.period_stat(Period::Weekly).await {
            info!("{:?}", ret);
        }
        if let Ok(ret) = self.period_stat(Period::Month).await {
            info!("{:?}", ret);
        }

        // object 查询 / api 调用情况
        if let Ok(ret) = self.meta_stat().await {
            info!("{:?}", ret);
        }

        let _ = self.report().await;

    }

    pub async fn get_desc(&self) -> BuckyResult<HashMap<u8, u64>> {
        let mut ret = HashMap::new();
        for i in 0..2 {
            let sum = self.storage.get_desc(i as u8).await?;
            ret.insert(i, sum);
        }
        Ok(ret)
    }
    
    // FIXME: 默认取当前日期
    pub async fn period_stat(&self, period: Period) -> BuckyResult<(HashMap<u8, u64>, HashMap<u8, u64>)> {
        let mut add = HashMap::new();
        for i in 0..2 {
            let sum = self.storage.get_desc_add(i as u8, period).await?;
            add.insert(i, sum);
        }

        let mut active = HashMap::new();
        for i in 0..2 {
            let sum = self.storage.get_desc_active(i as u8, period).await?;
            active.insert(i, sum);
        }

        Ok((add, active))
    }

    pub async fn meta_stat(&self) -> BuckyResult<(Vec<MetaStat>, Vec<MetaStat>)> {
        let v1 = self.storage.get_meta_stat(0u8, Period::Month).await?;
        let v2 = self.storage.get_meta_stat(1u8, Period::Month).await?;
        Ok((v1, v2))
    }

    pub async fn report(&self) -> BuckyResult<()> {
        let info = MonitorErrorInfo {
            service: "test".to_owned(),
            error: BuckyError::new(BuckyErrorCode::Failed, "test message"),
        };

        self.reporter.report(&info).await
    }

}