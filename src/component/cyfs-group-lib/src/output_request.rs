use cyfs_base::ObjectId;
use cyfs_lib::NONObjectInfo;

#[derive(Debug)]
pub struct GroupStartServiceOutputRequest {
    pub group_id: ObjectId,
    pub rpath: String,
}

pub struct GroupStartServiceOutputResponse {}

pub struct GroupPushProposalOutputResponse {
    pub object: Option<NONObjectInfo>,
}
