use cyfs_base::*;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::GroupPropsalDecideParam)]
pub struct GroupPropsalDecideParam {
    signature: Signature,
    proposal_id: ObjectId,
    decide: Vec<u8>,
}

impl GroupPropsalDecideParam {
    pub fn new(signature: Signature, proposal_id: ObjectId, decide: Vec<u8>) -> Self {
        Self {
            signature,
            proposal_id,
            decide,
        }
    }

    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    pub fn proposal_id(&self) -> &ObjectId {
        &self.proposal_id
    }

    pub fn decide(&self) -> &[u8] {
        &self.decide
    }
}

impl ProtobufTransform<crate::codec::protos::GroupPropsalDecideParam> for GroupPropsalDecideParam {
    fn transform(value: crate::codec::protos::GroupPropsalDecideParam) -> BuckyResult<Self> {
        Ok(Self {
            signature: Signature::raw_decode(value.signature.as_slice())?.0,
            proposal_id: ObjectId::transform(value.proposal_id)?,
            decide: value.decide,
        })
    }
}

impl ProtobufTransform<&GroupPropsalDecideParam> for crate::codec::protos::GroupPropsalDecideParam {
    fn transform(value: &GroupPropsalDecideParam) -> BuckyResult<Self> {
        Ok(Self {
            signature: value.signature.to_vec()?,
            proposal_id: value.proposal_id.to_vec()?,
            decide: value.decide.clone(),
        })
    }
}
