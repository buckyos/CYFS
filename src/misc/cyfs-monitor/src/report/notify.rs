use crate::def::*;
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

pub struct Notifier {
    // 基于钉钉机器人的报警功能
    dingtalk_url: String,
}

impl Notifier {
    pub fn new(dingtalk_url: &str) -> Self {
        Self {
            dingtalk_url: dingtalk_url.to_owned(),
        }
    }
    
    pub async fn report(&self, info: &MonitorErrorInfo) -> BuckyResult<()> {
        let content = format!("CYFS Monitor report: \nchannel:{}\nservice:{}\ncode:{:?}\nmsg:{}", get_channel(), info.service, info.error.code(), info.error.msg());

        let msg = serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": content,
            },
            "at": {
                "atMobiles": [],
                "isAtAll": true,
            }
        });

        let client = surf::client();
        let req = surf::post(&self.dingtalk_url).body(msg);

        let mut _res = client.send(req).await.map_err(|e|{
            let msg = format!("report to dingtalk error! {}", e);
            error!("{}", msg);
            BuckyError::from(msg)
        })?;

        info!("report to dingtalk success!");
        Ok(())
    }
}

#[async_trait::async_trait]
impl BugReporter for  Notifier {
    async fn report_error(&self, info: &MonitorErrorInfo) -> BuckyResult<()> {
        self.report(info).await
    }
}
