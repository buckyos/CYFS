use cyfs_base::ObjectId;
use cyfs_lib::NONObjectInfo;

pub struct GroupStartServiceInputRequest {
    pub group_id: ObjectId,
    pub rpath: String,
}

pub struct GroupStartServiceInputResponse {}

pub struct GroupPushProposalInputResponse {
    pub object: Option<NONObjectInfo>,
}
