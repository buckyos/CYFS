use super::notify::*;
use super::email::*;
use crate::Config;
use crate::def::*;
use cyfs_base::*;

pub struct StatReportManager {
    list: Vec<Box<dyn StatReporter>>,
}

impl StatReportManager {
    pub fn new(config: &Config) -> Self {
        let mut list: Vec<Box<dyn StatReporter>> = vec![];
        if let Some(email) = &config.email {
            let reporter = Lettre::new(email);
            list.push(Box::new(reporter));
        }
        if let Some(url) = &config.dingtalk_url {
            let reporter = Notifier::new(url.as_ref());
            list.push(Box::new(reporter));
        }

        Self { list }
    }

    pub async fn report(&self, info: &StatInfo) -> BuckyResult<()> {
        info!("will report error: {:?}", info);

        for item in &self.list {
            if let Err(e) = item.report_stat(info).await {
                error!("report error failed! {}", e);
            }
        }

        Ok(())
    }
}
