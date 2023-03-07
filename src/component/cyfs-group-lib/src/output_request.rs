use cyfs_base::ObjectId;

#[derive(Debug)]
pub struct GroupStartServiceOutputRequest {
    pub group_id: ObjectId,
    pub rpath: String,
}
