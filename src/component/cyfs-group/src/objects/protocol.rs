pub mod protos {
    include!(concat!(env!("OUT_DIR"), "/mod.rs"));
}

use cyfs_base::*;
use cyfs_core::{GroupRPath, HotstuffBlockQC};
use serde::Serialize;

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) enum HotstuffMessage {
    Block(cyfs_core::GroupConsensusBlock),
    BlockVote(HotstuffBlockQCVote),
    TimeoutVote(HotstuffTimeoutVote),
    Timeout(cyfs_core::HotstuffTimeout),
}

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) enum HotstuffPackage {
    Block(cyfs_core::GroupConsensusBlock),
    BlockVote(ProtocolAddress, HotstuffBlockQCVote),
    TimeoutVote(ProtocolAddress, HotstuffTimeoutVote),
    Timeout(ProtocolAddress, cyfs_core::HotstuffTimeout),
}

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) enum ProtocolAddress {
    Full(GroupRPath),
    Channel(u64),
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::protos::HotstuffBlockQcVote)]
pub(crate) struct HotstuffBlockQCVote {
    pub block_id: ObjectId,
    pub round: u64,
    pub dummy_round: u64,
    pub voter: ObjectId,
    pub signature: Signature,
}

impl ProtobufTransform<crate::protos::HotstuffBlockQcVote> for HotstuffBlockQCVote {
    fn transform(value: crate::protos::HotstuffBlockQcVote) -> BuckyResult<Self> {
        Ok(Self {
            voter: ObjectId::raw_decode(value.voter.as_slice())?.0,
            signature: Signature::raw_decode(value.signature.as_slice())?.0,
            block_id: ObjectId::raw_decode(value.block_id.as_slice())?.0,
            round: value.round,
            dummy_round: value.dummy_round,
        })
    }
}

impl ProtobufTransform<&HotstuffBlockQCVote> for crate::protos::HotstuffBlockQcVote {
    fn transform(value: &HotstuffBlockQCVote) -> BuckyResult<Self> {
        let ret = crate::protos::HotstuffBlockQcVote {
            block_id: value.block_id.to_vec()?,
            round: value.round,
            dummy_round: value.dummy_round,
            voter: value.voter.to_vec()?,
            signature: value.signature.to_vec()?,
        };

        Ok(ret)
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::protos::HotstuffTimeoutVote)]
pub(crate) struct HotstuffTimeoutVote {
    pub high_qc: HotstuffBlockQC,
    pub round: u64,
    pub voter: ObjectId,
    pub signature: Signature,
}

impl ProtobufTransform<crate::protos::HotstuffTimeoutVote> for HotstuffTimeoutVote {
    fn transform(value: crate::protos::HotstuffTimeoutVote) -> BuckyResult<Self> {
        Ok(Self {
            voter: ObjectId::raw_decode(value.voter.as_slice())?.0,
            signature: Signature::raw_decode(value.signature.as_slice())?.0,
            round: value.round,
            high_qc: HotstuffBlockQC::raw_decode(value.high_qc.as_slice())?.0,
        })
    }
}

impl ProtobufTransform<&HotstuffTimeoutVote> for crate::protos::HotstuffTimeoutVote {
    fn transform(value: &HotstuffTimeoutVote) -> BuckyResult<Self> {
        let ret = crate::protos::HotstuffTimeoutVote {
            high_qc: value.high_qc.to_vec()?,
            round: value.round,
            voter: value.voter.to_vec()?,
            signature: value.signature.to_vec()?,
        };

        Ok(ret)
    }
}

#[cfg(test)]
mod test {}
