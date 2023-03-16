use super::reporter::{ReportLogItem, CYFS_LOG_SESSION};
use crate::CyfsLogRecord;
use cyfs_base::*;

use futures::future::{self, AbortHandle, Aborted};
use std::error::Error;
use std::future::Future;
use std::sync::{Arc, Mutex};

const CONTENT_TYPE: &str = "Content-Type";
const CONTENT_LENGTH: &str = "Content-Length";


#[derive(Debug)]
pub struct LogRecordMeta {
    // 上报的client的此次session
    session_id: u64,

    // 关注的header字段
    headers: Vec<(String, Option<String>)>,
}

impl LogRecordMeta {
    pub fn headers(&self) -> Vec<(String, Option<String>)> {
        self.headers.clone()
    }
}

#[async_trait::async_trait]
pub trait OnRecvLogRecords: Send + Sync {
    async fn on_log_records(&self, meta: LogRecordMeta, list: Vec<ReportLogItem>) -> BuckyResult<()>;
}

#[async_trait::async_trait]
impl<F, Fut> OnRecvLogRecords for F
    where
        F: Send + Sync + 'static + Fn(LogRecordMeta, Vec<ReportLogItem>) -> Fut,
        Fut: Future<Output=BuckyResult<()>> + Send + 'static
{
    async fn on_log_records(&self, meta: LogRecordMeta, list: Vec<ReportLogItem>) -> BuckyResult<()> {
        let fut = (self)(meta, list);
        fut.await
    }
}

#[derive(Clone)]
pub struct HttpLogReceiver {
    listen_addr: String,
    canceller: Arc<Mutex<Option<AbortHandle>>>,
    server: tide::Server<()>,
}

struct HttpLogRecevierEndpoint {
    owner: HttpLogProcessor,
}

#[async_trait::async_trait]
impl<State> tide::Endpoint<State> for HttpLogRecevierEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: tide::Request<State>) -> tide::Result {
        let resp = match self.owner.process_request(req).await {
            Ok(()) => tide::Response::new(tide::StatusCode::Ok),
            Err(e) => {
                let mut resp = tide::Response::new(tide::StatusCode::BadRequest);
                resp.set_body(e.to_string());
                resp
            }
        };

        Ok(resp)
    }
}

#[derive(Clone)]
pub struct HttpLogProcessor {
    headers: Vec<String>,
    cb: Arc<dyn OnRecvLogRecords>,
}

impl HttpLogProcessor {
    pub fn new(
        headers: Vec<String>,
        callback: impl OnRecvLogRecords + 'static,
    ) -> Self {
        Self {
            headers,
            cb: Arc::new(callback),
        }
    }

    fn init_header<State>(
        name: &str,
        headers: &mut hyper::header::Headers,
        req: &tide::Request<State>,
    ) -> BuckyResult<()> {
        let v = req.header(name.to_lowercase().as_str());
        if v.is_none() {
            let msg = format!("{} header not found!", name);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let value = v.unwrap();
        let value: Vec<Vec<u8>> = value
            .iter()
            .map(|v| v.as_str().as_bytes().to_owned())
            .collect();
        headers.set_raw(name.to_owned(), value);

        Ok(())
    }

    fn extract_sid<State>(req: &tide::Request<State>) -> BuckyResult<u64> {
        let v = req.header(CYFS_LOG_SESSION);
        if v.is_none() {
            let msg = format!("{} header not found!", CYFS_LOG_SESSION);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let value = v.unwrap();
        let ret: u64 = value.last().as_str().parse().map_err(|e| {
            let msg = format!(
                "invalid session id header! {}, {}",
                value.last().as_str(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        Ok(ret)
    }

    fn extract_user_header<State>(name: &str, req: &tide::Request<State>) -> Option<String> {
        let v = req.header(name);
        if v.is_none() {
            return None;
        }

        let value = v.unwrap();
        Some(value.last().as_str().to_owned())
    }

    // 提取预定义的一些headers
    fn extract_user_headers<State>(
        &self,
        req: &tide::Request<State>,
    ) -> Vec<(String, Option<String>)> {
        let mut list = vec![];
        for name in &self.headers {
            let value = Self::extract_user_header(&name, req);
            list.push((name.clone(), value));
        }

        list
    }

    async fn process_request<State>(&self, mut req: tide::Request<State>) -> BuckyResult<()> {
        // println!("recv logs request");

        /*
        for (name, value) in req.iter() {
            println!("log header {}={}", name.as_str(), value.last().as_str());
        }
        */

        let mut headers = hyper::header::Headers::new();

        Self::init_header(CONTENT_TYPE, &mut headers, &req)?;
        Self::init_header(CONTENT_LENGTH, &mut headers, &req)?;

        let session_id = Self::extract_sid(&req)?;
        let body = req.body_bytes().await.map_err(|e| {
            let msg = format!("recv body bytes error! {}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let mut reader = std::io::Cursor::new(body);
        let data = formdata::read_formdata(&mut reader, &headers).map_err(|e| {
            // TODO Error的display和to_string实现有问题，会导致异常崩溃，所以这里只能暂时使用description来输出一些描述信息
            #[allow(deprecated)]
                let msg = format!("parse body formdata error! {:?}", e.description());
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        let headers = self.extract_user_headers(&req);

        // 解析record列表
        let mut list = Vec::with_capacity(data.fields.len());
        for (name, value) in data.fields {
            let record: CyfsLogRecord = serde_json::from_str(&value).map_err(|e| {
                let msg = format!("parse log record error! value={}, {}", value, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            let index: u64 = name.parse().map_err(|e| {
                let msg = format!(
                    "invalid log item index! {}, {}",
                    name,
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidParam, msg)
            })?;

            // println!("recv log {}: {}", name, record);
            let item = ReportLogItem { index, record };

            list.push(item);
        }

        let meta = LogRecordMeta {
            session_id,
            headers,
        };

        let ret = self.cb.on_log_records(meta, list).await;
        if ret.is_err() {
            error!("process received logs error! {}", ret.as_ref().unwrap_err());
        }

        ret
    }

    pub fn register(&self, server: &mut tide::Server<()>) {
        let ep = HttpLogRecevierEndpoint {
            owner: self.clone(),
        };

        server.at("/logs").post(ep);
    }
}

impl HttpLogReceiver {
    pub fn new(
        listen_addr: &str,
        headers: Vec<String>,
        callback: impl OnRecvLogRecords + 'static,
    ) -> Self {
        let mut server = tide::Server::new();
        let processor = HttpLogProcessor::new(headers, callback);
        processor.register(&mut server);
        Self {
            listen_addr: listen_addr.to_owned(),
            canceller: Arc::new(Mutex::new(None)),
            server
        }
    }

    pub fn start(&self) -> BuckyResult<()> {
        let this = self.clone();
        async_std::task::spawn(async move { this.run().await });

        Ok(())
    }

    async fn run(self) {
        let addr = self.listen_addr.clone();
        let (future, handle) = future::abortable(async move {
                self.server.listen(&addr).await.map_err(|e| {
                let msg = format!("logs server listen error! {}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::Failed, msg)
            })
        });

        // 保存abort_handle
        {
            let mut canceller = self.canceller.lock().unwrap();
            assert!(canceller.is_none());
            *canceller = Some(handle);
        }

        match future.await {
            Ok(_) => {
                info!("logs server finished complete: {}", self.listen_addr,);

                let mut canceller = self.canceller.lock().unwrap();
                assert!(canceller.is_some());
                *canceller = None;
            }
            Err(Aborted) => {
                info!("log server aborted: {}", self.listen_addr);
            }
        };
    }

    pub fn stop(&self) {
        if let Some(canceller) = self.canceller.lock().unwrap().take() {
            info!("will stop logs server: {}", self.listen_addr);
            canceller.abort();
        }
    }
}
