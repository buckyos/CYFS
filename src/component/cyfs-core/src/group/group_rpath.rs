use cyfs_base::*;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, PartialEq)]
#[cyfs_protobuf_type(crate::codec::protos::GroupRPath)]
pub struct GroupRPath {
    group_id: ObjectId,
    dec_id: ObjectId,
    r_path: String,
}

impl std::fmt::Debug for GroupRPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}-{:?}-{:?}", self.group_id, self.dec_id, self.r_path)
    }
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
