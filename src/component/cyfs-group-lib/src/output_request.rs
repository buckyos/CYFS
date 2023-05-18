use std::fmt::{self, Debug};

use cyfs_base::{NamedObject, ObjectDesc, ObjectId};
use cyfs_core::GroupProposal;
use cyfs_lib::NONObjectInfo;

#[derive(Clone, Debug)]
pub struct GroupOutputRequestCommon {
    // source dec-id
    pub dec_id: Option<ObjectId>,
}

impl GroupOutputRequestCommon {
    pub fn new() -> Self {
        Self { dec_id: None }
    }
}

impl fmt::Display for GroupOutputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(dec_id) = &self.dec_id {
            write!(f, ", dec_id: {}", dec_id)?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct GroupStartServiceOutputRequest {
    pub common: GroupOutputRequestCommon,
    pub group_id: ObjectId,
    pub rpath: String,
}

pub struct GroupStartServiceOutputResponse {}

pub struct GroupPushProposalOutputRequest {
    pub common: GroupOutputRequestCommon,
    pub proposal: GroupProposal,
}

impl Debug for GroupPushProposalOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GroupPushProposalOutputRequest")
            .field("common", &self.common)
            .field("proposal", &self.proposal.desc().object_id())
            .finish()
    }
}

pub struct GroupPushProposalOutputResponse {
    pub object: Option<NONObjectInfo>,
}
