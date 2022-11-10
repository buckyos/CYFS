use crate::{
    bug_report::{dingtalk_notify::DingtalkNotifier, HttpBugReporter},
    BugReportHandler, CyfsPanicInfo, DebugConfig,
};
use cyfs_base::*;

pub(crate) struct BugReportManager {
    list: Vec<Box<dyn BugReportHandler>>,
}

// only used on nightly env!!!
const DINGTALK_NIGHTLY_URL: &str = "https://oapi.dingtalk.com/robot/send?access_token=f44614438c8f63c7ccdd01ae1c83a062291e01b71b888aa21b7fa2b6588e4a9d";

impl BugReportManager {
    pub fn new() -> Self {
        let mut ret = Self { list: vec![] };

        ret.load_config();

        ret
    }

    fn load_config(&mut self) {
        if let Some(config_node) = DebugConfig::get_config("report") {
            if let Err(e) = self.load_config_value(config_node) {
                error!("load report config error! {}", e);
            }
        }

        if self.list.is_empty() && *get_channel() == CyfsChannel::Nightly {
            let reporter = DingtalkNotifier::new(DINGTALK_NIGHTLY_URL);
            self.list.push(Box::new(reporter));
        }
    }

    fn load_config_value(&mut self, config_node: &toml::Value) -> BuckyResult<()> {
        let node = config_node.as_table().ok_or_else(|| {
            let msg = format!("invalid dump config format! content={}", config_node,);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        for (k, v) in node {
            match k.as_str() {
                "http" => {
                    if let Some(v) = v.as_str() {
                        info!("load report.http from config: {}", v);

                        let reporter = HttpBugReporter::new(v);
                        self.list.push(Box::new(reporter));
                    } else {
                        error!("unknown report.http config node: {:?}", v);
                    }
                }
                "dingtalk" => {
                    if let Some(v) = v.as_str() {
                        info!("load report.dingtalk from config: {}", v);
                        let reporter = DingtalkNotifier::new(v);
                        self.list.push(Box::new(reporter));
                    } else {
                        error!("unknown report.dingtalk config node: {:?}", v);
                    }
                }

                key @ _ => {
                    error!("unknown report config node: {}={:?}", key, v);
                }
            }
        }

        Ok(())
    }
}

impl BugReportHandler for BugReportManager {
    fn notify(
        &self,
        product_name: &str,
        service_name: &str,
        panic_info: &CyfsPanicInfo,
    ) -> BuckyResult<()> {
        for reporter in &self.list {
            let _ = reporter.notify(product_name, service_name, panic_info);
        }

        Ok(())
    }
}
