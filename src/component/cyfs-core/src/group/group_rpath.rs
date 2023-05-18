use cyfs_base::*;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, PartialEq, Hash)]
#[cyfs_protobuf_type(crate::codec::protos::GroupRPath)]
pub struct GroupRPath {
    group_id: ObjectId,
    dec_id: ObjectId,
    rpath: String,
}

impl std::fmt::Debug for GroupRPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}-{:?}-{:?}", self.group_id, self.dec_id, self.rpath)
    }
}

impl GroupRPath {
    pub fn new(group_id: ObjectId, dec_id: ObjectId, rpath: String) -> Self {
        Self {
            group_id,
            dec_id,
            rpath,
        }
    }

    pub fn group_id(&self) -> &ObjectId {
        &self.group_id
    }

    pub fn dec_id(&self) -> &ObjectId {
        &self.dec_id
    }

    pub fn rpath(&self) -> &str {
        self.rpath.as_str()
    }
}
