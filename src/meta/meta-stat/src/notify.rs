use cyfs_base::*;
use crate::def::*;
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
    // 基于钉钉机器人的通知功能
    dingtalk_url: String,
}

impl Notifier {
    pub fn new(dingtalk_url: &str) -> Self {
        Self {
            dingtalk_url: dingtalk_url.to_owned(),
        }
    }
    
    pub async fn report(&self, info: &StatInfo) -> BuckyResult<()> {
        let content = format!("CYFS Stat report: \ncontext:{}", info.context);

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
impl StatReporter for  Notifier {
    async fn report_stat(&self, info: &StatInfo) -> BuckyResult<()> {
        self.report(info).await
    }
}