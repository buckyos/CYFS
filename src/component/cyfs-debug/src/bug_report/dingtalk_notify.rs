use super::request::PanicReportRequest;
use crate::panic::BugReportHandler;
use crate::panic::CyfsPanicInfo;
use cyfs_base::*;

/*
const NOTIFY_MSG: &str = r#"{
    "msgtype": "text",
    "text": {
        "content": ${content},
    },
    "at": {
        "atMobiles": [],
        "isAtAll": true
    }
};"#;
*/

#[derive(Clone)]
pub struct DingtalkNotifier {
    // 基于钉钉机器人的报警功能
    dingtalk_url: String,
}

impl DingtalkNotifier {
    pub fn new(dingtalk_url: &str) -> Self {
        Self {
            dingtalk_url: dingtalk_url.to_owned(),
        }
    }

    pub async fn notify(&self, req: PanicReportRequest) -> BuckyResult<()> {
        let content = format!(
            "CYFS service panic report: \nproduct:{}\nservice:{}\nbin:{}\nchannel:{}\ntarget:{}\nversion:{}\nmsg:{}",
            req.product_name,
            req.service_name,
            req.exe_name,
            req.channel,
            req.target,
            req.version,
            req.info_to_string(),
        );

        let at_all = match get_channel() {
            CyfsChannel::Nightly => false,
            _ => true,
        };

        let msg = serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": content,
            },
            "at": {
                "atMobiles": [],
                "isAtAll": at_all,
            }
        });

        let client = surf::client();
        let req = surf::post(&self.dingtalk_url).body(msg);

        let mut _res = client.send(req).await.map_err(|e| {
            let msg = format!("report to dingtalk error! {}", e);
            error!("{}", msg);
            BuckyError::from(msg)
        })?;

        info!("report to dingtalk success!");
        Ok(())
    }
}

impl BugReportHandler for DingtalkNotifier {
    fn notify(
        &self,
        product_name: &str,
        service_name: &str,
        panic_info: &CyfsPanicInfo,
    ) -> BuckyResult<()> {
        let req = PanicReportRequest::new(product_name, service_name, panic_info.to_owned());

        let this = self.clone();
        async_std::task::block_on(async move {
            let _ = this.notify(req).await;
        });

        Ok(())
    }
}
