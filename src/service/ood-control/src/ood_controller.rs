use super::request::*;
use cyfs_base::{BuckyError, BuckyResult, OOD_DAEMON_CONTROL_PORT};

use async_std::net::TcpStream;
use http_types::{Method, Mime, Request, Response, StatusCode, Url};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

pub struct OodController {
    url: Url,
    host: SocketAddr,
}

impl OodController {
    pub fn new(host: IpAddr) -> Self {
        let host = SocketAddr::new(host, OOD_DAEMON_CONTROL_PORT);
        let url = format!("http://{}", host.to_string());
        let url = Url::parse(&url).unwrap();

        Self { host, url }
    }

    pub async fn activate(&self, info: &ActivateInfo) -> BuckyResult<ActivateResult> {
        let url = self.url.join("activate").unwrap();

        let content = serde_json::to_string(info).unwrap();

        let mut req = Request::new(Method::Post, url);

        let mime = Mime::from_str("application/json").unwrap();
        req.set_content_type(mime);
        req.set_body(content);

        let mut resp = self.request(req).await?;

        let result = resp.body_json().await.map_err(|e| {
            let msg = format!("parse ood-daemon activate body error! err={}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        Ok(result)
    }

    pub async fn check(&self) -> BuckyResult<CheckResponse> {
        let url = self.url.join("activate").unwrap();
        let req = Request::new(Method::Get, url);

        let mut resp = self.request(req).await?;

        let result = resp.body_json().await.map_err(|e| {
            let msg = format!("parse ood-daemon check body error! err={}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        Ok(result)
    }

    async fn request(&self, req: Request) -> BuckyResult<Response> {
        let stream = TcpStream::connect(&self.host).await.map_err(|e| {
            error!(
                "tcp connect to ood-daemon control interface failed! host={}, err={}",
                self.host, e
            );
            BuckyError::from(e)
        })?;

        let resp = ::async_h1::connect(stream, req).await.map_err(|e| {
            error!(
                "http connect to ood-daemon control interface error! host={}, err={}",
                self.host, e
            );
            BuckyError::from(e)
        })?;

        if resp.status() != StatusCode::Ok {
            error!("ood-daemon control request resp status: {}", resp.status());
        }

        Ok(resp)
    }
}
