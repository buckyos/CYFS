use std::sync::Arc;

use cyfs_base::{
    BuckyError, BuckyResult, JsonCodec, NamedObject, ObjectDesc, ObjectId, RawConvertTo,
};
use cyfs_core::{GroupProposal, GroupProposalObject};
use cyfs_lib::{HttpRequestorRef, NONObjectInfo, NONRequestorHelper, RequestorHelper};
use http_types::{Method, Request, Url};

use crate::{
    output_request::GroupStartServiceOutputRequest,
    processor::{GroupOutputProcessor, GroupOutputProcessorRef},
    GroupOutputRequestCommon, GroupPushProposalOutputRequest, GroupPushProposalOutputResponse,
    GroupStartServiceOutputResponse,
};

#[derive(Clone)]
pub struct GroupRequestor {
    dec_id: ObjectId,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl GroupRequestor {
    pub fn new(dec_id: ObjectId, requestor: HttpRequestorRef) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/group/", addr);
        let url = Url::parse(&url).unwrap();

        Self {
            dec_id,
            requestor,
            service_url: url,
        }
    }

    pub fn clone_processor(&self) -> GroupOutputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    fn encode_common_headers(&self, com_req: &GroupOutputRequestCommon, http_req: &mut Request) {
        let dec_id = com_req.dec_id.as_ref().unwrap_or(&self.dec_id);
        http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
    }

    pub(crate) fn make_default_common(dec_id: ObjectId) -> GroupOutputRequestCommon {
        GroupOutputRequestCommon {
            dec_id: Some(dec_id),
        }
    }

    pub async fn start_service(
        &self,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse> {
        log::info!("will start group service: {:?}", req.rpath);

        let url = self.service_url.join("service").unwrap();
        let mut http_req = Request::new(Method::Put, url);
        self.encode_common_headers(&req.common, &mut http_req);

        let req = GroupStartServiceOutputRequest {
            group_id: req.group_id.clone(),
            rpath: req.rpath.to_string(),
            common: req.common,
        };

        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;

        match resp.status() {
            code if code.is_success() => {
                let body = resp.body_string().await.map_err(|e| {
                    let msg = format!(
                        "group start service failed, read body string error! req={:?} {}",
                        req, e
                    );
                    log::error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                // let resp = GroupStartServiceOutputResponse::decode_string(&body).map_err(|e| {
                //     error!(
                //         "decode group start service resp from body string error: body={} {}",
                //         body, e,
                //     );
                //     e
                // })?;

                log::debug!("group start service success");

                Ok(GroupStartServiceOutputResponse {})
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                log::error!(
                    "group start service failed: rpath={:?}, status={}, {}",
                    req.rpath,
                    code,
                    e
                );
                Err(e)
            }
        }
    }

    pub async fn push_proposal(
        &self,
        req: GroupPushProposalOutputRequest,
    ) -> BuckyResult<GroupPushProposalOutputResponse> {
        let proposal_id = req.proposal.desc().object_id();
        log::info!(
            "will push proposal: {:?}, {}",
            req.proposal.rpath(),
            proposal_id
        );

        let url = self.service_url.join("proposal").unwrap();
        let mut http_req = Request::new(Method::Put, url);

        self.encode_common_headers(&req.common, &mut http_req);

        NONRequestorHelper::encode_object_info(
            &mut http_req,
            NONObjectInfo::new(proposal_id, req.proposal.to_vec()?, None),
        );

        let mut resp = self.requestor.request(http_req).await?;

        let status = resp.status();
        if status.is_success() {
            match status {
                http_types::StatusCode::NoContent => {
                    let e = RequestorHelper::error_from_resp(&mut resp).await;
                    log::info!(
                        "push proposal but empty response! obj={}, {}",
                        proposal_id,
                        e
                    );
                    Err(e)
                }
                _ => {
                    log::info!("push proposal success: {}", proposal_id);
                    let object = NONRequestorHelper::decode_option_object_info(&mut resp).await?;
                    let ret = GroupPushProposalOutputResponse { object };
                    Ok(ret)
                }
            }
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            log::error!("push proposal error! object={}, {}", proposal_id, e);
            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl GroupOutputProcessor for GroupRequestor {
    async fn start_service(
        &self,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse> {
        GroupRequestor::start_service(self, req).await
    }

    async fn push_proposal(
        &self,
        req: GroupPushProposalOutputRequest,
    ) -> BuckyResult<GroupPushProposalOutputResponse> {
        GroupRequestor::push_proposal(self, req).await
    }
}
