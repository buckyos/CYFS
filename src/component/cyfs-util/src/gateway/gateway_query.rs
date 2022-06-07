use async_std::net::TcpStream;
use cyfs_base::{BuckyError, BuckyErrorCode, ObjectId};
use http_types::{Method, Request, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub struct GatewayQuery {
    host: Url,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayPeerAssocQueryResult {
    pub code: String,
    pub msg: String,
    pub peer_id: Option<String>,
}

impl GatewayQuery {
    pub fn new() -> Self {
        Self {
            host: Url::parse(super::GATEWAY_CONTROL_URL).unwrap(),
        }
    }

    pub async fn query_assoc_peerid(
        &self,
        protocol: &str,
        port: u16,
    ) -> Result<ObjectId, BuckyError> {
        let url = self.host.join("peer_assoc").unwrap();

        let body = format!(r#"{{ "protocol": "{}", "port": "{}" }}"#, protocol, port,);

        let req = Request::new(Method::Get, url);

        match self.post(req, body).await {
            Ok(ret) => {
                if ret.code == "0" {
                    assert!(ret.peer_id.is_some());

                    debug!(
                        "query peer assoc success! {} -> {}",
                        port,
                        ret.peer_id.as_ref().unwrap()
                    );
                    Ok(ObjectId::from_str(&ret.peer_id.as_ref().unwrap()).unwrap())
                } else {
                    let msg = format!("query peer assoc error! ret={:?}", ret);
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
    ) -> Result<GatewayPeerAssocQueryResult, BuckyError> {
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

        match resp.status() {
            StatusCode::Ok => resp.body_json().await.map_err(|e| {
                let msg = format!("parse gateway register resp body error! err={}", e);
                error!("{}", msg);

                BuckyError::from(msg)
            }),
            StatusCode::NotFound => {
                warn!("query assoc peerid but not found!");
                Err(BuckyError::from(BuckyErrorCode::NotFound))
            }
            StatusCode::BadRequest => {
                error!("query assoc peerid with invalid format!");
                Err(BuckyError::from(BuckyErrorCode::InvalidFormat))
            }
            v @ _ => {
                let msg = format!("query assoc peerid error! status={}", v);
                error!("{}", msg);
                Err(BuckyError::from(msg))
            }
        }
    }
}
