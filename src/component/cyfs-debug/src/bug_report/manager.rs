use crate::{
    bug_report::{dingtalk_notify::DingtalkNotifier, HttpBugReporter},
    BugReportHandler, CyfsPanicInfo, DebugConfig,
};
use cyfs_base::*;

pub(crate) struct BugReportManager {
    list: Vec<Box<dyn BugReportHandler>>,
}

impl BugReportManager {
    pub fn new() -> Self {
        let mut ret = Self { list: vec![] };

        ret.load_config();

        ret
    }

    fn load_config(&mut self) {
        if let Some(config_node) = DebugConfig::get_config("report") {
            if let Err(e) = self.load_config_value(config_node) {
                println!("load report config error! {}", e);
            }
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
                        println!("load report.http from config: {}", v);

                        let reporter = HttpBugReporter::new(v);
                        self.list.push(Box::new(reporter));
                    } else {
                        println!("unknown report.http config node: {:?}", v);
                    }
                }
                "dingtalk" => {
                    if let Some(v) = v.as_str() {
                        println!("load report.dingtalk from config: {}", v,);
                        let reporter = DingtalkNotifier::new(v);
                        self.list.push(Box::new(reporter));
                    } else {
                        println!("unknown report.dingtalk config node: {:?}", v);
                    }
                }

                key @ _ => {
                    println!("unknown report config node: {}={:?}", key, v);
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
