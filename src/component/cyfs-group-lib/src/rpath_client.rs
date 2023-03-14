use std::sync::Arc;

use cyfs_base::{BuckyResult, NamedObject, ObjectDesc, ObjectId, RawConvertTo};
use cyfs_core::{GroupProposal, GroupProposalObject, GroupRPath};
use cyfs_lib::{
    NONAPILevel, NONObjectInfo, NONOutputRequestCommon, NONPostObjectOutputRequest, NONRequestor,
};

struct RPathClientRaw {
    rpath: GroupRPath,
    local_dec_id: Option<ObjectId>,
    requestor: NONRequestor,
}

#[derive(Clone)]
pub struct RPathClient(Arc<RPathClientRaw>);

impl RPathClient {
    pub(crate) fn new(
        rpath: GroupRPath,
        local_dec_id: Option<ObjectId>,
        requestor: NONRequestor,
    ) -> Self {
        Self(Arc::new(RPathClientRaw {
            requestor,
            rpath,
            local_dec_id,
        }))
    }

    pub fn rpath(&self) -> &GroupRPath {
        &self.0.rpath
    }

    // post proposal to the admins, it's same as calling to non.post_object with default parameters;
    // and you can call the non.post_object with more parameters.
    pub async fn post_proposal(
        &self,
        proposal: &GroupProposal,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        self.0
            .requestor
            .post_object(NONPostObjectOutputRequest {
                common: NONOutputRequestCommon {
                    req_path: Some("post-proposal".to_string()),
                    source: None,
                    dec_id: self.0.local_dec_id.clone(),
                    level: NONAPILevel::Router,
                    target: Some(proposal.rpath().group_id().clone()),
                    flags: 0,
                },
                object: NONObjectInfo::new(proposal.desc().object_id(), proposal.to_vec()?, None),
            })
            .await
            .map(|resp| resp.object)
    }
}
