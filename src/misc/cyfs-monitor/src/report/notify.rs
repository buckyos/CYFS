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
        let content = format!(r#"
CYFS Monitor report:
    channel: {}
    service: {}
    case: {}
    code: {:?}
    msg: {}
"#, get_channel(), &info.service, &info.case, info.error.code(), info.error.msg());

        let msg = serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": content,
            },
            "at": {
                "atMobiles": [],
                "isAtAll": info.at_all,
            }
        });

        let _ = surf::post(&self.dingtalk_url).body(msg).await.map_err(|e|{
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
