use super::{super::protocol::*, ping_status::PingStatus};
use cyfs_base::*;
use cyfs_lib::*;

use http_types::{Method, Request, Response, StatusCode, Url};
use std::sync::{RwLock, Arc};

mod ip_helper {
    use std::net::{Ipv4Addr, SocketAddr};

    fn is_global(ip: &Ipv4Addr) -> bool {
        // check if this address is 192.0.0.9 or 192.0.0.10. These addresses are the only two
        // globally routable addresses in the 192.0.0.0/24 range.
        if u32::from_be_bytes(ip.octets()) == 0xc0000009
            || u32::from_be_bytes(ip.octets()) == 0xc000000a
        {
            return true;
        }

        !ip.is_private()
            && !ip.is_loopback()
            && !ip.is_link_local()
            && !ip.is_broadcast()
            && !ip.is_documentation()
            //&& !ip.is_shared()
            //&& !ip.is_ietf_protocol_assignment()
            //&& !ip.is_reserved()
            //&& !ip.is_benchmarking()
            // Make sure the address is not in 0.0.0.0/8
            && ip.octets()[0] != 0
    }

    pub fn is_local_v4(addr: &SocketAddr) -> bool {
        match addr {
            SocketAddr::V4(addr) => !is_global(addr.ip()),
            _ => false,
        }
    }
}

pub(crate) struct SyncClientRequestor {
    requestor: RwLock<Arc<Box<dyn HttpRequestor>>>,
    service_url: Url,
}

impl SyncClientRequestor {
    pub fn new(requestor: Box<dyn HttpRequestor>) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/sync/", addr);
        let service_url = Url::parse(&url).unwrap();

        Self {
            requestor: RwLock::new(Arc::new(requestor)),
            service_url,
        }
    }

    fn encode_ping_request(&self, req: &SyncPingRequest) -> Request {
        let p = format!("ping/{}", req.device_id.to_string());
        let url = self.service_url.join(&p).unwrap();

        let mut http_req = Request::new(Method::Post, url);
        http_req.set_content_type(tide::http::mime::JSON);
        http_req.set_body(req.encode_string());

        http_req
    }

    pub(crate) fn requestor(&self) -> Arc<Box<dyn HttpRequestor>> {
        self.requestor.read().unwrap().clone()
    }

    pub fn reset_requestor(&self, requestor: Box<dyn HttpRequestor>) {
        *self.requestor.write().unwrap() = Arc::new(requestor);
    }

    pub async fn ping(
        &self,
        req: SyncPingRequest,
        ping_status: &PingStatus,
    ) -> BuckyResult<SyncPingResponse> {
        let http_req = self.encode_ping_request(&req);

        debug!("sync will ping: {:?}", req);

        let begin = bucky_time_now();

        let mut conn_info = HttpRequestConnectionInfo::None;
        let mut resp = self
            .requestor()
            .request_with_conn_info(http_req, Some(&mut conn_info))
            .await
            .map_err(|e| {
                ping_status.on_ping_failed(e.code());
                e
            })?;

        // 统计连接的类型
        let mut network = OODNetworkType::Extranet;
        match conn_info {
            HttpRequestConnectionInfo::Bdt((local, remote)) => {
                // 目前只有双方都是内网ipv4地址，才认为是内网在线
                if ip_helper::is_local_v4(local.addr()) && ip_helper::is_local_v4(remote.addr()) {
                    network = OODNetworkType::Intranet;
                }

                debug!(
                    "ping network via bdt: local={}, remote={}, type={}",
                    local, remote, network
                );
            }
            HttpRequestConnectionInfo::Tcp((local, remote)) => {
                // 目前只有双方都是内网ipv4地址，才认为是内网在线
                if ip_helper::is_local_v4(&local) && ip_helper::is_local_v4(&remote) {
                    network = OODNetworkType::Intranet;
                }

                debug!(
                    "ping network via tcp: local={}, remote={}, type={}",
                    local, remote, network
                );
            }
            _ => {
                unreachable!();
            }
        };

        // 统计此次ping结果
        let during = bucky_time_now() - begin;
        ping_status.on_ping_success(network, BuckyErrorCode::Ok, during);

        match resp.status() {
            StatusCode::Ok => {
                let body = resp.body_string().await.map_err(|e| {
                    let msg = format!(
                        "sync ping failed, read body string error! req={:?} {}",
                        req, e
                    );
                    error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                let ping_resp = SyncPingResponse::decode_string(&body).map_err(|e| {
                    error!(
                        "decode ping resp from body string error: body={} {}",
                        body, e,
                    );
                    e
                })?;

                debug!("sync ping success: resp={:?}", ping_resp);

                Ok(ping_resp)
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!("ping resp failed: req={:?}, status={} err={}", req, code, e);

                Err(e)
            }
        }
    }

    fn encode_diff_request(&self, req: &SyncDiffRequest) -> Request {
        let url = self.service_url.join("diff").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        http_req.set_content_type(::tide::http::mime::JSON);
        http_req.set_body(req.encode_string());

        http_req
    }

    async fn decode_diff_response(resp: Response) -> BuckyResult<SyncDiffResponse> {
        let revision = RequestorHelper::decode_header(&resp, cyfs_base::CYFS_REVISION)?;
        let target = RequestorHelper::decode_optional_header(&resp, cyfs_base::CYFS_TARGET)?;
        
        let objects = if target.is_some() {
            match SyncObjectsResponse::from_respone(resp.into()).await {
                Ok(objects_resp) => {
                    debug!("sync diff got objects: resp={}", objects_resp);
    
                    objects_resp.objects
                }
                Err(e) => {
                    error!("decode sync diff objects from resp error: {}", e,);
                    return Err(e);
                }
            }
        } else {
            vec![]
        };

        Ok(SyncDiffResponse {
            revision,
            target,
            objects,
        })
    }

    pub async fn sync_diff(
        &self,
        req: SyncDiffRequest,
    ) -> BuckyResult<SyncDiffResponse> {
        let http_req = self.encode_diff_request(&req);

        debug!("sync will diff: {:?}", req);

        let mut resp = self.requestor().request_timeout(http_req, std::time::Duration::from_secs(30)).await?;

        match resp.status() {
            StatusCode::Ok => {
                Self::decode_diff_response(resp).await
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "sync diff resp failed: req={:?}, status={} err={}",
                    req, code, e
                );

                Err(e)
            }
        }
    }

    fn encode_objects_request(&self, req: &SyncObjectsRequest) -> Request {
        let url = self.service_url.join("objects").unwrap();

        let mut http_req = Request::new(Method::Get, url);
        http_req.set_content_type(::tide::http::mime::JSON);
        http_req.set_body(req.encode_string());

        http_req
    }

    pub async fn sync_objects(&self, req: SyncObjectsRequest) -> BuckyResult<SyncObjectsResponse> {
        let http_req = self.encode_objects_request(&req);

        debug!("sync objects: {:?}", req);

        // TODO 是否存在对象很大，导致五分钟都传不完的情况？这里先限定最大时长为5分钟
        let mut resp = self.requestor().request_timeout(http_req, std::time::Duration::from_secs(60 * 5)).await?;

        match resp.status() {
            StatusCode::Ok => match SyncObjectsResponse::from_respone(resp).await {
                Ok(objects_resp) => {
                    debug!("sync objects success: resp={}", objects_resp);

                    Ok(objects_resp)
                }
                Err(e) => {
                    error!("decode objects resp error: {}", e,);
                    Err(e)
                }
            },
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "sync objects resp failed: req={:?}, status={} err={}",
                    req, code, e
                );

                Err(e)
            }
        }
    }

    fn encode_chunks_request(&self, req: &SyncChunksRequest) -> Request {
        let url = self.service_url.join("chunks").unwrap();

        let mut http_req = Request::new(Method::Get, url);
        http_req.set_content_type(::tide::http::mime::JSON);
        http_req.set_body(req.encode_string());

        http_req
    }

    pub async fn sync_chunks(&self, req: &SyncChunksRequest) -> BuckyResult<SyncChunksResponse> {
        let http_req = self.encode_chunks_request(&req);

        debug!("sync chunks: {:?}", req);

        let mut resp = self.requestor().request_timeout(http_req, std::time::Duration::from_secs(60 * 2)).await?;

        match resp.status() {
            StatusCode::Ok => match RequestorHelper::decode_json_body(&mut resp).await {
                Ok(chunks_resp) => {
                    debug!("sync chunks success: resp={:?}", chunks_resp);

                    Ok(chunks_resp)
                }
                Err(e) => {
                    error!("decode chunks resp error: {}", e);
                    Err(e)
                }
            },
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "sync chunks resp failed: req={:?}, status={} err={}",
                    req, code, e
                );

                Err(e)
            }
        }
    }
}
