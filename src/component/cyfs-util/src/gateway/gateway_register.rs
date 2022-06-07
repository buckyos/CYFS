use async_std::net::TcpStream;
use async_std::prelude::*;
use cyfs_base::BuckyError;
use http_types::{Method, Request, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct GatewayRegister {
    pub id: String,
    pub server_type: String,
    pub value: String,

    pub host: Url,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayRegisterResult {
    pub code: String,
    pub msg: String,
}

impl GatewayRegister {
    pub fn new(id: String, server_type: String) -> GatewayRegister {
        GatewayRegister {
            id,
            server_type,
            value: "".to_owned(),
            host: Url::parse(super::GATEWAY_CONTROL_URL).unwrap(),
        }
    }

    pub fn register(id: String, server_type: String, value: String) -> Result<(), BuckyError> {
        let _: serde_json::Value = serde_json::from_str(&value).map_err(|e| {
            let msg = format!("invalid register value format! {}, err={}", value, e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        assert!(server_type == "http" || server_type == "stream");
        assert!(id.len() > 0);

        let mut register = GatewayRegister::new(id, server_type);
        register.value = value;

        // 一次注册失败，并不会返回错误？
        async_std::task::spawn(async move {
            register.run_register().await;
        });

        Ok(())
    }

    pub async fn unregister(id: String, server_type: String) -> Result<(), BuckyError> {
        assert!(server_type == "http" || server_type == "stream");
        assert!(id.len() > 0);

        let register = GatewayRegister::new(id, server_type);

        register.unregister_once().await
    }

    pub async fn run_register(self) {
        let _r = self.register_once().await;

        // 超时时间为60s，这里我们每45秒注册一次
        let mut interval = async_std::stream::interval(Duration::from_secs(45));
        while let Some(_) = interval.next().await {
            let _r = self.register_once().await;
        }
    }

    async fn register_once(&self) -> Result<(), BuckyError> {
        let url = self.host.join("register").unwrap();

        let body = format!(
            r#"{{ "id": "{}", "type": "{}", "value": {} }} "#,
            self.id, self.server_type, self.value
        );

        let req = Request::new(Method::Post, url);

        match self.post(req, body).await {
            Ok(ret) => {
                if ret.code == "0" {
                    debug!("{} register to gateway success!", self.id);
                    Ok(())
                } else {
                    let msg = format!("register to gateway error! ret={:?}", ret);
                    error!("{}", msg);

                    Err(BuckyError::from(msg))
                }
            }
            Err(e) => Err(e),
        }
    }

    async fn unregister_once(&self) -> Result<(), BuckyError> {
        let url = self.host.join("unregister").unwrap();

        let body = format!(
            r#"{{ "id": "{}", "type": "{}" }} "#,
            self.id, self.server_type
        );

        let req = Request::new(Method::Post, url);

        match self.post(req, body).await {
            Ok(ret) => {
                if ret.code == "0" {
                    debug!("register to gateway success! id={}", self.id);
                    Ok(())
                } else {
                    let msg = format!("register to gateway error! ret={:?}", ret);
                    error!("{}", msg);

                    Err(BuckyError::from(msg))
                }
            }
            Err(e) => Err(e),
        }
    }

    async fn post(
        &self,
        mut req: Request,
        body: String,
    ) -> Result<GatewayRegisterResult, BuckyError> {
        let host = self.host.host_str().unwrap();
        let port = self.host.port().unwrap();
        let addr = format!("{}:{}", host, port);

        let stream = TcpStream::connect(addr).await.map_err(|e| {
            error!(
                "tcp connect to gateway control interface failed! host={}, err={}",
                self.host, e
            );
            BuckyError::from(e)
        })?;

        req.set_body(body);

        let mut resp = ::async_h1::connect(stream, req).await.map_err(|e| {
            error!(
                "http connect to gateway control interface error! host={}, err={}",
                self.host, e
            );
            BuckyError::from(e)
        })?;

        if resp.status() != StatusCode::Ok {
            error!("gateway register resp status: {}", resp.status());
        }

        resp.body_json().await.map_err(|e| {
            let msg = format!("parse gateway register resp body error! err={}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })
    }
}
