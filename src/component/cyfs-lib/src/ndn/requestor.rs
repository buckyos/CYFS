use super::def::*;
use super::output_request::*;
use super::processor::*;
use crate::base::*;
use crate::stack::SharedObjectStackDecID;
use cyfs_base::*;

use http_types::{Method, Request, Response, Url};
use std::borrow::Cow;
use std::sync::Arc;

pub struct NDNRequestorHelper;

impl NDNRequestorHelper {
    pub async fn decode_get_data_response(
        resp: &mut Response,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        let data = Box::new(resp.take_body());

        let attr: Option<u32> =
            RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_ATTRIBUTES)?;
        let attr = attr.map(|v| Attributes::new(v));

        let object_id = RequestorHelper::decode_header(resp, cyfs_base::CYFS_OBJECT_ID)?;
        let owner_id = RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_OWNER_ID)?;

        let range = RequestorHelper::decode_optional_json_header(resp, cyfs_base::CYFS_DATA_RANGE)?;

        let length: u64 =
            RequestorHelper::decode_header(resp, http_types::headers::CONTENT_LENGTH)?;
        let ret = NDNGetDataOutputResponse {
            object_id,
            owner_id,
            attr,
            
            range,

            length,
            data,
        };

        Ok(ret)
    }
}


#[derive(Clone)]
pub struct NDNRequestor {
    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl NDNRequestor {
    pub fn new_default_tcp(dec_id: Option<SharedObjectStackDecID>) -> Self {
        let service_addr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_HTTP_PORT);
        Self::new_tcp(dec_id, &service_addr)
    }

    pub fn new_tcp(dec_id: Option<SharedObjectStackDecID>, service_addr: &str) -> Self {
        let tcp_requestor = TcpHttpRequestor::new(service_addr);
        Self::new(dec_id, Arc::new(Box::new(tcp_requestor)))
    }

    pub fn new(dec_id: Option<SharedObjectStackDecID>, requestor: HttpRequestorRef) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/ndn/", addr);
        let url = Url::parse(&url).unwrap();

        Self {
            dec_id,
            requestor,
            service_url: url,
        }
    }

    pub fn into_processor(self) -> NDNOutputProcessorRef {
        Arc::new(Box::new(self))
    }

    pub fn clone_processor(&self) -> NDNOutputProcessorRef {
        self.clone().into_processor()
    }

    // url??????????????????????????????device_id?????????
    // {host:port}/ndn/[req_path/]object_id[/inner_path]
    fn format_url(
        &self,
        req_path: Option<&String>,
        object_id: Option<&ObjectId>,
        inner_path: Option<&String>,
    ) -> Url {
        let mut parts = vec![];
        if let Some(req_path) = req_path {
            parts.push(Cow::Borrowed(
                req_path
                    .as_str()
                    .trim_start_matches('/')
                    .trim_end_matches('/'),
            ));
        }

        if let Some(object_id) = object_id {
            let object_id = object_id.to_string();
            parts.push(Cow::Owned(object_id));
        }

        if let Some(inner_path) = inner_path {
            parts.push(Cow::Borrowed(
                inner_path
                    .as_str()
                    .trim_start_matches('/')
                    .trim_end_matches('/'),
            ));
        }

        let p = parts.join("/");
        self.service_url.join(&p).unwrap()
    }

    fn encode_common_headers(
        &self,
        action: NDNAction,
        com_req: &NDNOutputRequestCommon,
        http_req: &mut Request,
    ) {
        if let Some(dec_id) = &com_req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        } else if let Some(dec_id) = &self.dec_id {
            if let Some(dec_id) = dec_id.get() {
                http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }
        }

        http_req.insert_header(cyfs_base::CYFS_NDN_ACTION, action.to_string());

        http_req.insert_header(cyfs_base::CYFS_API_LEVEL, com_req.level.to_string());

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        if !com_req.referer_object.is_empty() {
            let headers: Vec<String> = com_req
                .referer_object
                .iter()
                .map(|v| v.to_string())
                .collect();

            RequestorHelper::insert_headers(http_req, cyfs_base::CYFS_REFERER_OBJECT, &headers);
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());
    }

    fn encode_put_data_request(&self, req: &NDNPutDataOutputRequest) -> Request {
        let url = self.format_url(req.common.req_path.as_ref(), Some(&req.object_id), None);

        let mut http_req = Request::new(Method::Put, url);

        self.encode_common_headers(NDNAction::PutData, &req.common, &mut http_req);

        http_req
    }

    async fn decode_put_data_response(
        &self,
        resp: &Response,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        let result: NDNPutDataResult =
            RequestorHelper::decode_header(resp, cyfs_base::CYFS_RESULT)?;

        let ret = NDNPutDataOutputResponse { result };

        Ok(ret)
    }

    #[allow(unused_mut)]
    pub async fn put_data(
        &self,
        mut req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        let mut http_req = self.encode_put_data_request(&req);

        #[cfg(debug_assertions)]
        {
            use async_std::io::ReadExt;

            let mut data = Vec::new();
            req.data.read_to_end(&mut data).await.map_err(|e| {
                let msg = format!("read data failed! chunk={} {}", req.object_id, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

            if data.len() != req.length as usize {
                error!(
                    "chunk length unmatch: calc={}, expect={}",
                    data.len(),
                    req.length,
                );
                unreachable!();
            }

            let calc_id = ChunkId::calculate_sync(&data).unwrap();

            if calc_id.object_id() != req.object_id {
                error!(
                    "chunk id unmatch: calc_id={}, expect={}",
                    calc_id, req.object_id,
                );
                unreachable!();
            }

            http_req.set_body(data);
        }
        #[cfg(not(debug_assertions))]
        {
            let reader = async_std::io::BufReader::new(req.data);
            let body = tide::Body::from_reader(reader, Some(req.length as usize));
            http_req.set_body(body);
        }
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!("put data to ndn service success: {}", req.object_id);
            self.decode_put_data_response(&resp).await
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "put data to ndn service error! object={}, {}",
                req.object_id, e
            );
            Err(e)
        }
    }

    fn encode_put_shared_data_request(&self, req: &NDNPutDataOutputRequest) -> Request {
        let url = self.format_url(req.common.req_path.as_ref(), Some(&req.object_id), None);

        let mut http_req = Request::new(Method::Put, url);

        self.encode_common_headers(NDNAction::PutSharedData, &req.common, &mut http_req);

        http_req
    }

    async fn decode_put_shared_data_response(
        &self,
        resp: &Response,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        let result: NDNPutDataResult =
            RequestorHelper::decode_header(resp, cyfs_base::CYFS_RESULT)?;

        let ret = NDNPutDataOutputResponse { result };

        Ok(ret)
    }

    pub async fn put_shared_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        let mut http_req = self.encode_put_shared_data_request(&req);

        let reader = async_std::io::BufReader::new(req.data);
        let body = tide::Body::from_reader(reader, Some(req.length as usize));
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!("put shared data to ndn service success: {}", req.object_id);
            self.decode_put_shared_data_response(&resp).await
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "put shared data to ndn service error! object={}, {}",
                req.object_id, e
            );
            Err(e)
        }
    }

    fn encode_get_data_request(&self, req: &NDNGetDataOutputRequest) -> Request {
        let url = self.format_url(
            req.common.req_path.as_ref(),
            Some(&req.object_id),
            req.inner_path.as_ref(),
        );

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(NDNAction::GetData, &req.common, &mut http_req);

        if let Some(ref range) = req.range {
            http_req.insert_header("Range", range.encode_string());
        }

        http_req
    }

    pub async fn get_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        let http_req = self.encode_get_data_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            
            match NDNRequestorHelper::decode_get_data_response(&mut resp).await {
                Ok(resp) => {
                    info!("get data from ndn service success: {}", resp);
                    Ok(resp)
                }
                Err(e) => {
                    error!("decode get data response error: {}, {}", req.object_id, e);
                    Err(e)
                }
            }
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "get data from ndn service error: object={}, {}",
                req.object_id, e
            );
            Err(e)
        }
    }

    fn encode_get_shared_data_request(&self, req: &NDNGetDataOutputRequest) -> Request {
        let url = self.format_url(
            req.common.req_path.as_ref(),
            Some(&req.object_id),
            req.inner_path.as_ref(),
        );

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(NDNAction::GetSharedData, &req.common, &mut http_req);

        http_req
    }

    async fn decode_get_shared_data_response(
        &self,
        _req: &NDNGetDataOutputRequest,
        resp: &mut Response,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        let data = Box::new(resp.take_body());

        let attr: Option<u32> =
            RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_ATTRIBUTES)?;
        let attr = attr.map(|v| Attributes::new(v));

        let object_id = RequestorHelper::decode_header(resp, cyfs_base::CYFS_OBJECT_ID)?;
        let owner_id = RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_OWNER_ID)?;

        let range = RequestorHelper::decode_optional_json_header(resp, cyfs_base::CYFS_DATA_RANGE)?;

        let length: u64 =
            RequestorHelper::decode_header(resp, http_types::headers::CONTENT_LENGTH)?;

        let ret = NDNGetDataOutputResponse {
            object_id,
            owner_id,

            attr,

            range,

            length,
            data,
        };

        Ok(ret)
    }

    pub async fn get_shared_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        let http_req = self.encode_get_shared_data_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!("get data from ndn service success: {}", req.object_id);
            self.decode_get_shared_data_response(&req, &mut resp).await
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "get data from ndn service error: object={}, {}",
                req.object_id, e
            );
            Err(e)
        }
    }

    fn encode_delete_data_request(&self, req: &NDNDeleteDataOutputRequest) -> Request {
        let url = self.format_url(
            req.common.req_path.as_ref(),
            Some(&req.object_id),
            req.inner_path.as_ref(),
        );

        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(NDNAction::DeleteData, &req.common, &mut http_req);

        http_req
    }

    async fn decode_delete_data_response(
        &self,
        resp: &Response,
    ) -> BuckyResult<NDNDeleteDataOutputResponse> {
        let object_id = RequestorHelper::decode_header(resp, cyfs_base::CYFS_OBJECT_ID)?;

        let ret = NDNDeleteDataOutputResponse { object_id };

        Ok(ret)
    }

    pub async fn delete_data(
        &self,
        req: NDNDeleteDataOutputRequest,
    ) -> BuckyResult<NDNDeleteDataOutputResponse> {
        let http_req = self.encode_delete_data_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!("delete data from ndn service success: {}", req.object_id);
            self.decode_delete_data_response(&resp).await
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "delete data from ndn service error! object={}, {}",
                req.object_id, e
            );
            Err(e)
        }
    }

    fn encode_query_file_request(&self, req: &NDNQueryFileOutputRequest) -> Request {
        let mut url = self.format_url(req.common.req_path.as_ref(), None, None);

        let (t, v) = req.param.to_key_pair();
        url.query_pairs_mut()
            .append_pair("type", t)
            .append_pair("value", &v);

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(NDNAction::QueryFile, &req.common, &mut http_req);

        http_req
    }

    async fn decode_query_file_response(
        &self,
        resp: &mut Response,
    ) -> BuckyResult<NDNQueryFileOutputResponse> {
        let ret: NDNQueryFileOutputResponse = RequestorHelper::decode_json_body(resp).await?;

        Ok(ret)
    }

    async fn query_file(
        &self,
        req: NDNQueryFileOutputRequest,
    ) -> BuckyResult<NDNQueryFileOutputResponse> {
        let http_req = self.encode_query_file_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            // info!("query file from ndn service success: {}", resp);
            self.decode_query_file_response(&mut resp).await
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "query file from ndn service error! param={}, {}",
                req.param, e
            );
            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl NDNOutputProcessor for NDNRequestor {
    async fn put_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        self.put_data(req).await
    }

    async fn get_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        self.get_data(req).await
    }

    async fn put_shared_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        self.put_shared_data(req).await
    }

    async fn get_shared_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        self.get_shared_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataOutputRequest,
    ) -> BuckyResult<NDNDeleteDataOutputResponse> {
        self.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileOutputRequest,
    ) -> BuckyResult<NDNQueryFileOutputResponse> {
        self.query_file(req).await
    }
}
