use std::fmt;

use cyfs_base::ObjectId;
use cyfs_core::GroupProposal;
use cyfs_lib::{NONObjectInfo, RequestSourceInfo};

#[derive(Clone, Debug)]
pub struct GroupInputRequestCommon {
    // the request source info in bundle
    pub source: RequestSourceInfo,
}

impl fmt::Display for GroupInputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ", {}", self.source)?;

        Ok(())
    }
}

pub struct GroupStartServiceInputRequest {
    pub common: GroupInputRequestCommon,
    pub group_id: ObjectId,
    pub rpath: String,
}

pub struct GroupStartServiceInputResponse {}

pub struct GroupPushProposalInputRequest {
    pub common: GroupInputRequestCommon,
    pub proposal: GroupProposal,
}

pub struct GroupPushProposalInputResponse {
    pub object: Option<NONObjectInfo>,
}
