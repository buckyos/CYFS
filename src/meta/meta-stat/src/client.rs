use std::sync::Arc;
use std::collections::HashMap;
use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode};
use crate::notify::*;
use crate::storage::{Storage, Period, MetaStat};
use comfy_table::Table;

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
        let mut table = Table::new();
        table.set_header(vec!["People", "Device"]);
        if let Ok(ret) = self.get_desc().await {
            let ret: Vec<u64> = ret.into_iter().map(|v| v.1).collect();
            table.add_row(ret);
            println!("{table}");
        }
        let mut table1 = Table::new();
        table1.set_header(vec!["People Add", "Device Add"]);

        let mut table2 = Table::new();
        table2.set_header(vec!["People Active", "Device Active"]);

        // 日/周/月表
        if let Ok(ret) = self.period_stat(Period::Daily).await {
            let add: Vec<u64> = ret.0.into_iter().map(|v| v.1).collect();
            let active: Vec<u64> = ret.1.into_iter().map(|v| v.1).collect();

            table1.add_row(add);
            
            table2.add_row(active);
        }
        if let Ok(ret) = self.period_stat(Period::Weekly).await {
            let add: Vec<u64> = ret.0.into_iter().map(|v| v.1).collect();
            let active: Vec<u64> = ret.1.into_iter().map(|v| v.1).collect();

            table1.add_row(add);
            
            table2.add_row(active);
        }
        if let Ok(ret) = self.period_stat(Period::Month).await {
            let add: Vec<u64> = ret.0.into_iter().map(|v| v.1).collect();
            let active: Vec<u64> = ret.1.into_iter().map(|v| v.1).collect();

            table1.add_row(add);
            
            table2.add_row(active);
        }

        println!("{table1}");
        println!("{table2}");

        let mut table3 = Table::new();
        table3.set_header(vec!["Meta Object", "Success", "Failed"]);

        let mut table4 = Table::new();
        table4.set_header(vec!["Meta Api", "Success", "Failed"]);
        // object 查询 / api 调用情况
        if let Ok(ret) = self.meta_stat().await {
            for v in ret.0.into_iter() {
                table3.add_row(vec![v.id, v.success.to_string(), v.failed.to_string()]);
            }

            for v in ret.1.into_iter() {
                table4.add_row(vec![v.id, v.success.to_string(), v.failed.to_string()]);
            }
        }
        println!("{table3}");
        println!("{table4}");

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