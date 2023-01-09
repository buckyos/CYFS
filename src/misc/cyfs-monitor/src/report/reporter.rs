use super::local::*;
use super::notify::*;
use crate::def::*;
use crate::monitor::MontiorConfig;
use cyfs_base::*;

pub struct BugReportManager {
    list: Vec<Box<dyn BugReporter>>,
}

impl BugReportManager {
    pub fn new(config: &MontiorConfig) -> Self {
        let mut list: Vec<Box<dyn BugReporter>> = vec![];
        let reporter = LocalStore::new();
        list.push(Box::new(reporter));

        if let Some(url) = &config.dingtalk_url {
            info!("reporter use dingtalk url {}", url);
            let reporter = Notifier::new(url.as_ref());
            list.push(Box::new(reporter));
        }

        Self { list }
    }

    pub async fn report(&self, info: &MonitorErrorInfo) -> BuckyResult<()> {
        info!("will report error: {:?}", info);

        for item in &self.list {
            if let Err(e) = item.report_error(info).await {
                error!("report error failed! {}", e);
            }
        }

        Ok(())
    }
}
