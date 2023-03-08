use std::sync::Arc;

use cyfs_base::{
    BuckyError, BuckyResult, JsonCodec, NamedObject, ObjectDesc, ObjectId, RawConvertTo,
    CYFS_API_LEVEL,
};
use cyfs_core::{GroupProposal, GroupProposalObject};
use cyfs_lib::{
    HttpRequestorRef, NONObjectInfo, NONOutputRequestCommon, NONRequestorHelper, RequestorHelper,
};
use http_types::{Method, Request, Url};

use crate::{
    output_request::GroupStartServiceOutputRequest,
    processor::{GroupOutputProcessor, GroupOutputProcessorRef},
    GroupPushProposalOutputResponse, GroupStartServiceOutputResponse,
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

    fn encode_common_headers(
        &self,
        // action: NONAction,
        com_req: &NONOutputRequestCommon,
        http_req: &mut Request,
    ) {
        let dec_id = com_req.dec_id.as_ref().unwrap_or(&self.dec_id);
        http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());

        RequestorHelper::encode_opt_header_with_encoding(
            http_req,
            cyfs_base::CYFS_REQ_PATH,
            com_req.req_path.as_deref(),
        );

        // http_req.insert_header(cyfs_base::CYFS_NON_ACTION, action.to_string());

        http_req.insert_header(CYFS_API_LEVEL, com_req.level.to_string());

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        if let Some(source) = &com_req.source {
            http_req.insert_header(cyfs_base::CYFS_SOURCE, source.to_string());
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());
    }

    pub(crate) fn make_default_common(dec_id: ObjectId) -> NONOutputRequestCommon {
        NONOutputRequestCommon {
            req_path: None,
            source: None,
            dec_id: Some(dec_id),
            level: cyfs_lib::NONAPILevel::NOC,
            target: None,
            flags: 0,
        }
    }

    pub async fn start_service(
        &self,
        req_common: NONOutputRequestCommon,
        group_id: &ObjectId,
        rpath: &str,
    ) -> BuckyResult<GroupStartServiceOutputResponse> {
        log::info!("will start group service: {:?}", rpath);

        let url = self.service_url.join("start-service").unwrap();
        let mut http_req = Request::new(Method::Put, url);

        let req = GroupStartServiceOutputRequest {
            group_id: group_id.clone(),
            rpath: rpath.to_string(),
        };

        self.encode_common_headers(&req_common, &mut http_req);
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
                    rpath,
                    code,
                    e
                );
                Err(e)
            }
        }
    }

    pub async fn push_proposal(
        &self,
        req_common: NONOutputRequestCommon,
        proposal: &GroupProposal,
    ) -> BuckyResult<GroupPushProposalOutputResponse> {
        let proposal_id = proposal.desc().object_id();
        log::info!(
            "will push proposal: {:?}, {}",
            proposal.rpath(),
            proposal_id
        );

        let url = self.service_url.join("push-proposal").unwrap();
        let mut http_req = Request::new(Method::Put, url);

        self.encode_common_headers(&req_common, &mut http_req);

        NONRequestorHelper::encode_object_info(
            &mut http_req,
            NONObjectInfo::new(proposal_id, proposal.to_vec()?, None),
        );

        let mut resp = self.requestor.request(http_req).await?;

        match resp.status() {
            code if code.is_success() => {
                let body = resp.body_string().await.map_err(|e| {
                    let msg = format!(
                        "group push proposal failed, read body string error! req={:?}/{} {}",
                        proposal.rpath(),
                        proposal_id,
                        e
                    );
                    log::error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                // let resp = GroupPushProposalOutputResponse::decode_string(&body).map_err(|e| {
                //     error!(
                //         "decode group push proposal resp from body string error: body={} {}",
                //         body, e,
                //     );
                //     e
                // })?;

                log::debug!(
                    "group push proposal success, req={:?}/{}",
                    proposal.rpath(),
                    proposal_id
                );

                Ok(GroupPushProposalOutputResponse {})
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                log::error!(
                    "group push proposal failed: rpath={:?}/{}, status={}, {}",
                    proposal.rpath(),
                    proposal_id,
                    code,
                    e
                );
                Err(e)
            }
        }
    }
}

#[async_trait::async_trait]
impl GroupOutputProcessor for GroupRequestor {
    async fn start_service(
        &self,
        req_common: NONOutputRequestCommon,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse> {
        GroupRequestor::start_service(self, req_common, &req.group_id, req.rpath.as_str()).await
    }

    async fn push_proposal(
        &self,
        req_common: NONOutputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalOutputResponse> {
        GroupRequestor::push_proposal(self, req_common, &req).await
    }
}
