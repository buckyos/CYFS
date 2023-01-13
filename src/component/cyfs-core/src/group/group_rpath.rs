use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize, PartialEq)]
#[cyfs_protobuf_type(crate::codec::protos::GroupRPath)]
pub struct GroupRPath {
    group_id: ObjectId,
    dec_id: ObjectId,
    r_path: String,
}

impl GroupRPath {
    pub fn new(group_id: ObjectId, dec_id: ObjectId, r_path: String) -> Self {
        Self {
            group_id,
            dec_id,
            r_path,
        }
    }

    pub fn group_id(&self) -> &ObjectId {
        &self.group_id
    }

    pub fn dec_id(&self) -> &ObjectId {
        &self.dec_id
    }

    pub fn r_path(&self) -> &str {
        self.r_path.as_str()
    }
}
