use std::sync::Arc;
use std::collections::HashMap;
use cyfs_base::BuckyResult;

use crate::storage::{Storage, Period};

pub struct Client {
    storage: Arc<Box<dyn Storage + Send + Sync>>,
    deadline: u16,
}

impl Client {
    pub(crate) fn new(deadline: u16, storage: Arc<Box<dyn Storage + Send + Sync>>) -> Self {
        Self {
            storage,
            deadline,
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

        // web hook dingding/email

        todo!()
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

    pub async fn meta_object_stat(&self) -> BuckyResult<()> {
        todo!()
    }

    pub async fn meta_api_stat(&self) -> BuckyResult<()> {
        todo!()
    }

    pub async fn report(&self) -> BuckyResult<()> {
        Ok(())
    }

}