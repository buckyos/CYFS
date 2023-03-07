use cyfs_base::ObjectId;

pub struct GroupStartServiceInputRequest {
    pub group_id: ObjectId,
    pub rpath: String,
}

pub struct GroupStartServiceInputResponse {}

pub struct GroupPushProposalInputResponse {}
