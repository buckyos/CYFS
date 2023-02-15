use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::GroupUpdateGroupPropsalParam)]
pub struct GroupUpdateGroupPropsalParam {
    target_dec_id: Vec<ObjectId>,
    from_chunk_id: Option<ObjectId>,
    to_chunk_id: ObjectId,
}

impl GroupUpdateGroupPropsalParam {
    pub fn new(
        target_dec_id: Vec<ObjectId>,
        from_chunk_id: Option<ObjectId>,
        to_chunk_id: ObjectId,
    ) -> Self {
        Self {
            target_dec_id,
            from_chunk_id,
            to_chunk_id,
        }
    }

    pub fn target_dec_id(&self) -> &[ObjectId] {
        self.target_dec_id.as_slice()
    }

    pub fn from_chunk_id(&self) -> &Option<ObjectId> {
        &self.from_chunk_id
    }

    pub fn to_chunk_id(&self) -> &ObjectId {
        &self.to_chunk_id
    }
}
