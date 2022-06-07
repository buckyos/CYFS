use async_h1::client;
use async_std::net::TcpStream;
use http_types::{
    headers::{CONTENT_LENGTH, CONTENT_TYPE},
    Method, Request, Response, StatusCode, Url,
};
use std::sync::{Arc, Mutex};

use super::http_forward::HTTP_FORWARD_MANAGER;
use cyfs_base::BuckyError;

#[derive(Debug, Clone)]
pub(super) struct HttpListenerBase {
    forwards: Vec<u32>,
}

impl HttpListenerBase {
    pub fn new() -> HttpListenerBase {
        HttpListenerBase {
            forwards: Vec::new(),
        }
    }

    pub fn forward_count(&self) -> usize {
        self.forwards.len()
    }

    pub fn bind_forward(&mut self, forward_id: u32) {
        assert!(forward_id > 0);
        assert!(!self.forwards.iter().any(|x| *x == forward_id));

        self.forwards.push(forward_id);
    }

    pub fn unbind_forward(&mut self, forward_id: u32) -> bool {
        assert!(forward_id > 0);

        for i in 0..self.forwards.len() {
            if self.forwards[i] == forward_id {
                self.forwards.remove(i);

                return true;
            }
        }

        false
    }

    pub async fn dispatch_request(base: &Arc<Mutex<HttpListenerBase>>, req: Request) -> Response {
        let mut resp = Self::dispatch_request_impl(base, req).await;

        info!(
            "resp body: status={:?}, len={:?}",
            resp.status(),
            resp.len()
        );
        resp.remove_header(&CONTENT_LENGTH);

        return resp;
    }

    async fn dispatch_request_impl(base: &Arc<Mutex<HttpListenerBase>>, req: Request) -> Response {
        let mut proxy_pass: Option<String> = None;

        let forwards;
        {
            let base = base.lock().unwrap();
            forwards = base.forwards.clone();
        }

        {
            let forward_manager = HTTP_FORWARD_MANAGER.lock().unwrap();

            for forward_id in forwards {
                let ret = forward_manager.get_forward(&forward_id);
                if ret.is_none() {
                    continue;
                }

                let ret = ret.unwrap();
                let forward = ret.lock().unwrap();
                let dispatch_ret = forward.find_dispatch(&req);
                if dispatch_ret.is_none() {
                    continue;
                }

                proxy_pass = dispatch_ret;
                break;
            }
        }

        if proxy_pass.is_none() {
            return Response::new(StatusCode::NotFound);
        }

        let proxy_pass = Url::parse(&proxy_pass.unwrap()).unwrap();

        let resp = HttpListenerBase::proxy_pass(req, proxy_pass).await;
        if resp.is_err() {
            let mut res = Response::new(StatusCode::InternalServerError);
            let _ret = res.insert_header("Content-Type", "text/plain");
            let msg = format!("{}", resp.unwrap_err());
            res.set_body(msg);
            return res;
        }

        let resp = resp.unwrap();
        return resp;
    }

    async fn proxy_pass(mut req: Request, target_url: Url) -> Result<Response, BuckyError> {
        let host = target_url.host_str().unwrap_or("localhost");
        let port = target_url.port().unwrap_or(80);

        let addr = format!("{}:{}", host, port);
        info!("will deal with proxy_pass: {}, url={}", addr, target_url);

        let stream = TcpStream::connect(&addr).await;
        if stream.is_err() {
            let e = stream.unwrap_err();
            error!(
                "connect by pass target error, proxy_pass={}, err={}",
                target_url, e
            );

            return Err(BuckyError::from(e));
        }

        // 修正request
        if let Err(e) = HttpListenerBase::fix_request(&mut req, &target_url) {
            error!("fix request error, proxy_pass={}, err={}", target_url, e);

            return Err(BuckyError::from(e));
        }

        info!("will forward request to {}, req={:?}", target_url, req);

        let stream = stream.unwrap();
        let resp = client::connect(stream, req).await;
        if resp.is_err() {
            let e = resp.unwrap_err();
            error!(
                "open http connection error, proxy_pass={}, err={}",
                target_url, e
            );

            return Err(BuckyError::from(e));
        }

        info!("by pass recv resp! proxy_pass={}", target_url);
        Ok(resp.unwrap())
    }

    fn fix_request(req: &mut Request, target_url: &Url) -> Result<(), BuckyError> {
        let req_url = req.url_mut();
        req_url.set_host(target_url.host_str())?;
        req_url.set_path(target_url.path());
        req_url.set_query(target_url.query());
        if let Err(_) = req_url.set_scheme(target_url.scheme()) {
            error!("set scheme error! target_url={}", target_url);
            return Err(BuckyError::from("scheme error"));
        }

        req.remove_header(&CONTENT_LENGTH);

        if req.method() == Method::Get {
            req.remove_header(&CONTENT_TYPE);
        }
        req.insert_header("connection", "close");

        // req.insert_header("accept", "text/html,application/xhtml+xml,application/xml");
        // req.insert_header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.138 Safari/537.36");
        // req.insert_header("accept-encoding", "gzip, deflate, br");
        // req.insert_header("accept-language", "zh-CN,zh;q=0.9,en;q=0.8,zh-TW;q=0.7");
        // req.remove_header(&CONTENT_LENGTH);
        // req.remove_header(&CONTENT_TYPE);
        // req.insert_header("connection", "keep-alive");

        Ok(())
    }
}
