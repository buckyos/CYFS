use cyfs_base::*;
use crate::{CyfsLogRecord, CyfsLogTarget};

use formdata::FormData;
use std::collections::LinkedList;
use std::sync::{Arc, Mutex};
use surf::http::mime;
use url::Url;

pub struct ReportLogItem {
    pub index: u64,
    pub record: CyfsLogRecord,
}

struct LogCache {
    // 最大容量
    capacity: usize,
    next_index: u64,
    pending: LinkedList<ReportLogItem>,
}

impl LogCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_index: 0,
            pending: LinkedList::new(),
        }
    }

    pub fn add(&mut self, record: CyfsLogRecord) {
        let index = self.next_index + 1;
        self.next_index += 1;
        if self.next_index == u64::MAX {
            self.next_index = 0;
        }
        let item = ReportLogItem { index, record };
        self.pending.push_back(item);

        self.shrink();
    }

    pub fn shrink(&mut self) {
        while self.pending.len() > self.capacity {
            let item = self.pending.pop_front().unwrap();
            println!("will drop log item: {}", item.index);
        }
    }

    pub fn fetch(&mut self, count: usize) -> Vec<ReportLogItem> {
        let mut list = Vec::with_capacity(count);
        for _ in 0..count {
            let ret = self.pending.pop_front();
            if ret.is_none() {
                break;
            }

            list.push(ret.unwrap());
        }

        list
    }

    pub fn restore(&mut self, mut list: Vec<ReportLogItem>) {
        while !list.is_empty() {
            let item = list.pop().unwrap();
            if self.pending.len() < self.capacity {
                self.pending.push_front(item);
            } else {
                break;
            }
        }
    }
}

#[derive(Clone)]
pub struct HttpLogReporter {
    // 每次启动后，会随机一个session
    session_id: u64,

    headers: Vec<(String, String)>,
    service_url: Url,
    pending_logs: Arc<Mutex<LogCache>>,
}

const REPORT_DEFAULT_INTERVAL_SECS: u64 = 5;
const REPORT_MAX_INTERVAL_SECS: u64 = 60;

pub const CYFS_LOG_SESSION: &str = "cyfs-log-session";

// 采用固定的boundary
const BOUNDARY: &str = "ZCJVFU8RF2IAAAAAA7VkZHelG4MgsRTVKG0gi2ETgMTNZggxml+t+1GQIcJRKNPiG+YG";

impl HttpLogReporter {
    pub fn new(service_url: Url, headers: Vec<(String, String)>, mut capacity: usize) -> Self {
        if capacity == 0 {
            capacity = 1024 * 10;
        }

        let pending_logs = LogCache::new(capacity);

        Self {
            session_id: rand::random::<u64>(),
            headers,
            service_url,
            pending_logs: Arc::new(Mutex::new(pending_logs)),
        }
    }

    pub fn start(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            // 随机一个时间开始上报
            let r = rand::random::<u64>() % 30;
            async_std::task::sleep(std::time::Duration::from_secs(r)).await;

            this.run().await;
        });
    }

    async fn run(self) {
        let mut interval_secs = REPORT_DEFAULT_INTERVAL_SECS;
        loop {
            match self.report_once().await {
                Ok(()) => {
                    interval_secs = REPORT_DEFAULT_INTERVAL_SECS;
                }
                Err(_) => {
                    interval_secs *= 2;
                    if interval_secs > REPORT_MAX_INTERVAL_SECS {
                        interval_secs = REPORT_MAX_INTERVAL_SECS;
                    }
                }
            }

            async_std::task::sleep(std::time::Duration::from_secs(interval_secs)).await;
        }
    }

    async fn report_once(&self) -> BuckyResult<()> {
        let list = self.fetch();
        if list.is_empty() {
            return Ok(());
        }

        let ret = self.report_list(&list).await;

        if ret.is_err() {
            let mut cache = self.pending_logs.lock().unwrap();
            cache.restore(list);
        }

        ret
    }

    async fn report_list(&self, list: &Vec<ReportLogItem>) -> BuckyResult<()> {
        let body = Self::encode(&list);
        let mut body_buffer = vec![];

        // boundary里面不能包含/, 会导致解析mime-multipart这个库对header失败，但这里生成的随机值里面又可能包含这个字符，所以这里我们使用固定的分隔符
        // let boundary = formdata::generate_boundary();
        // let error_boundary = "DKMUKF8RF2IAAAAAaHrLKCEUXT+JK+s8QT/eIjQnmHABo6q7Rrp00cmZ5ZHuXzyBw7nH";
        
        let _count = formdata::write_formdata(&mut body_buffer, &BOUNDARY.as_bytes().to_owned(), &body).unwrap();

        let mut builder = surf::post(&self.service_url)
            .body(body_buffer)
            .content_type(mime::MULTIPART_FORM)
            .header(CYFS_LOG_SESSION, self.session_id.to_string())
            .header("Content-Type", format!("multipart/form-data; boundary={}", BOUNDARY));

        // 添加外部header
        for (k, v) in &self.headers {
            builder = builder.header(k.as_str(), v);
        }

        let req = builder.build();

        let client = surf::client();
        let res = client.send(req).await.map_err(|e| {
            let msg = format!("report logs but request error! server={}, {}", self.service_url, e.to_string());
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;

        if !res.status().is_success() {
            let msg = format!(
                "report logs but got error response: code={:?},",
                res.status()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        Ok(())
    }

    fn encode(list: &Vec<ReportLogItem>) -> FormData {
        let fields: Vec<(String, String)> = list
            .iter()
            .map(|item| {
                (
                    item.index.to_string(),
                    serde_json::to_string(&item.record).unwrap(),
                )
            })
            .collect();

        FormData {
            fields,
            files: vec![],
        }
    }

    fn fetch(&self) -> Vec<ReportLogItem> {
        let mut cache = self.pending_logs.lock().unwrap();
        cache.fetch(1024)
    }
}

impl CyfsLogTarget for HttpLogReporter {
    fn log(&self, record: &CyfsLogRecord) {
        let mut cache = self.pending_logs.lock().unwrap();
        cache.add(record.to_owned());
    }
}
