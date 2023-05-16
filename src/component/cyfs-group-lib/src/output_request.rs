use std::fmt;

use cyfs_base::{DeviceId, ObjectId};
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
    pub group_id: ObjectId,
    pub rpath: String,
}

pub struct GroupStartServiceOutputResponse {}

pub struct GroupPushProposalOutputResponse {
    pub object: Option<NONObjectInfo>,
}
