use crate::upstream::{AssociationProtocol, PEER_ASSOC_MANAGER};
use cyfs_base::{BuckyError, BuckyErrorCode};
use tide::{Response, StatusCode};

struct AssocRequest {
    protocol: AssociationProtocol,
    port: u16,
}

pub struct AssocServer;

impl AssocServer {
    pub fn query(req_body: &str) -> Response {
        match Self::parse_req(req_body) {
            Ok(req) => {
                match PEER_ASSOC_MANAGER
                    .lock()
                    .unwrap()
                    .query(&req.protocol, &req.port)
                {
                    Some(peer_id) => {
                        let mut resp = Response::new(StatusCode::Ok);
                        let body = format!(
                            r#"{{"code": "0", "msg": "Ok", "peer_id": "{}"}}"#,
                            peer_id.to_string()
                        );
                        resp.set_body(body);

                        resp
                    }
                    None => {
                        warn!(
                            "query peer assoc but not found: protocol={}, port={}",
                            req.protocol, req.port
                        );

                        let mut resp = Response::new(StatusCode::NotFound);
                        let body = format!(r#"{{"code": "1", "msg": "NotFound" }}"#);
                        resp.set_body(body);

                        resp
                    }
                }
            }
            Err(e) => {
                let mut resp = Response::new(StatusCode::BadRequest);
                let body = format!(r#"{{"code": "{:?}", "msg": "{}"}}"#, e.code(), e.msg());
                resp.set_body(body);

                resp
            }
        }
    }

    fn parse_req(req_body: &str) -> Result<AssocRequest, BuckyError> {
        let node: ::serde_json::Value = ::serde_json::from_str(req_body).map_err(|e| {
            let msg = format!("load value as hjson error! value={}, err={}", req_body, e);
            error!("{}", msg);

            BuckyError::from((BuckyErrorCode::InvalidFormat, msg))
        })?;

        if !node.is_object() {
            let msg = format!("invalid value format, not object! value={}", req_body);
            error!("{}", msg);

            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        let node = node.as_object().unwrap();

        let protocol = match node.get("protocol") {
            Some(v) => v.as_str().unwrap_or(""),
            None => "",
        };

        let port = match node.get("port") {
            Some(v) => v.as_str().unwrap_or(""),
            None => "",
        };

        if protocol.len() == 0 || port.len() == 0 {
            let msg = format!("invalid protocol or port! value={}", req_body);
            error!("{}", msg);

            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        // 解析端口
        let port = port.parse::<u16>().map_err(|e| {
            let msg = format!(
                "invalid port format, not u16 number! value={}, err={}",
                port, e
            );
            warn!("{}", msg);

            BuckyError::from((BuckyErrorCode::InvalidFormat, msg))
        })?;

        // 解析协议
        let protocol = AssociationProtocol::from(protocol)?;

        Ok(AssocRequest { protocol, port })
    }
}
