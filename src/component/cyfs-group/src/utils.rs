use cyfs_base::ObjectId;

pub type Round = u64;

pub enum IsCreateRPath {
    No,
    Yes(Option<ObjectId>),
}
