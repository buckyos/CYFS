use std::sync::Arc;
use std::collections::HashMap;
use cyfs_base::BuckyResult;

use crate::storage::Storage;

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
        // 日表
        // 周表
        // 月表
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

    pub async fn daily_stat(&self) -> BuckyResult<()> {
        todo!()
    }

    pub async fn weekly_stat(&self) -> BuckyResult<()> {
        Ok(())
    }

    pub async fn monthly_stat(&self) -> BuckyResult<()> {
        Ok(())
    }

    pub async fn report(&self) -> BuckyResult<()> {
        Ok(())
    }

}