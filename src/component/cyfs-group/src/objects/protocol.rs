pub mod protos {
    include!(concat!(env!("OUT_DIR"), "/mod.rs"));
}

use cyfs_base::*;
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupRPath, GroupRPathStatus, HotstuffBlockQC,
};
use cyfs_lib::NONObjectInfo;
use sha2::Digest;

#[derive(RawEncode, RawDecode, PartialEq, Eq, Ord, Clone)]
pub enum SyncBound {
    Height(u64),
    Round(u64),
}

impl Copy for SyncBound {}

impl SyncBound {
    pub fn value(&self) -> u64 {
        match self {
            Self::Height(h) => *h,
            Self::Round(r) => *r,
        }
    }

    pub fn height(&self) -> u64 {
        match self {
            Self::Height(h) => *h,
            Self::Round(r) => panic!("should be height"),
        }
    }

    pub fn round(&self) -> u64 {
        match self {
            Self::Round(r) => *r,
            Self::Height(h) => panic!("should be round"),
        }
    }

    pub fn add(&self, value: u64) -> Self {
        match self {
            Self::Height(h) => Self::Height(*h + value),
            Self::Round(r) => Self::Round(*r + value),
        }
    }

    pub fn sub(&self, value: u64) -> Self {
        match self {
            Self::Height(h) => Self::Height(*h - value),
            Self::Round(r) => Self::Round(*r - value),
        }
    }
}

impl PartialOrd for SyncBound {
    fn partial_cmp(&self, other: &SyncBound) -> Option<std::cmp::Ordering> {
        let ord = match self {
            Self::Height(height) => match other {
                Self::Height(other_height) => height.cmp(other_height),
                Self::Round(other_round) => {
                    if height >= other_round {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Less
                    }
                }
            },
            Self::Round(round) => match other {
                Self::Round(other_round) => round.cmp(other_round),
                Self::Height(other_height) => {
                    if other_height >= round {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                }
            },
        };

        Some(ord)
    }
}

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) enum HotstuffMessage {
    Block(cyfs_core::GroupConsensusBlock),
    BlockVote(HotstuffBlockQCVote),
    TimeoutVote(HotstuffTimeoutVote),
    Timeout(cyfs_core::HotstuffTimeout),

    SyncRequest(SyncBound, SyncBound),

    LastStateRequest,
    StateChangeNotify(GroupConsensusBlock, GroupConsensusBlock), // (block, qc-block)
    ProposalResult(ObjectId, BuckyResult<Option<NONObjectInfo>>), // (proposal-id, ExecuteResult)
    QueryState(String),
    VerifiableState(GroupRPathStatus),
}

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) enum HotstuffPackage {
    Block(cyfs_core::GroupConsensusBlock),
    BlockVote(ProtocolAddress, HotstuffBlockQCVote),
    TimeoutVote(ProtocolAddress, HotstuffTimeoutVote),
    Timeout(ProtocolAddress, cyfs_core::HotstuffTimeout),

    SyncRequest(SyncBound, SyncBound),

    StateChangeNotify(ProtocolAddress, GroupConsensusBlock, GroupConsensusBlock), // (block, qc-block)
    ProposalResult(
        ProtocolAddress,
        ObjectId,
        BuckyResult<Option<NONObjectInfo>>,
    ), // (proposal-id, ExecuteResult)
    QueryState(ProtocolAddress, String),
    VerifiableState(ProtocolAddress, GroupRPathStatus),
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
    pub prev_block_id: Option<ObjectId>,
    pub round: u64,
    pub voter: ObjectId,
    pub signature: Signature,
}

impl HotstuffBlockQCVote {
    pub async fn new(
        block: &GroupConsensusBlock,
        local_id: ObjectId,
        signer: &RsaCPUObjectSigner,
    ) -> BuckyResult<Self> {
        let block_id = block.named_object().desc().object_id();
        let round = block.round();
        let signature = signer
            .sign(
                Self::hash_content(&block_id, block.prev_block_id(), round).as_slice(),
                &SignatureSource::RefIndex(0),
            )
            .await?;

        Ok(Self {
            block_id,
            round,
            voter: local_id,
            signature,
            prev_block_id: block.prev_block_id().map(|id| id.clone()),
        })
    }

    fn hash(&self) -> HashValue {
        Self::hash_content(&self.block_id, self.prev_block_id.as_ref(), self.round)
    }

    fn hash_content(
        block_id: &ObjectId,
        prev_block_id: Option<&ObjectId>,
        round: u64,
    ) -> HashValue {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(block_id.as_slice());
        sha256.input(round.to_le_bytes());
        if let Some(prev_block_id) = prev_block_id {
            sha256.input(prev_block_id.as_slice());
        }
        sha256.result().into()
    }
}

impl ProtobufTransform<crate::protos::HotstuffBlockQcVote> for HotstuffBlockQCVote {
    fn transform(value: crate::protos::HotstuffBlockQcVote) -> BuckyResult<Self> {
        Ok(Self {
            voter: ObjectId::raw_decode(value.voter.as_slice())?.0,
            signature: Signature::raw_decode(value.signature.as_slice())?.0,
            block_id: ObjectId::raw_decode(value.block_id.as_slice())?.0,
            round: value.round,
            prev_block_id: match value.prev_block_id.as_ref() {
                Some(id) => Some(ObjectId::raw_decode(id.as_slice())?.0),
                None => None,
            },
        })
    }
}

impl ProtobufTransform<&HotstuffBlockQCVote> for crate::protos::HotstuffBlockQcVote {
    fn transform(value: &HotstuffBlockQCVote) -> BuckyResult<Self> {
        let ret = crate::protos::HotstuffBlockQcVote {
            block_id: value.block_id.to_vec()?,
            round: value.round,
            voter: value.voter.to_vec()?,
            signature: value.signature.to_vec()?,
            prev_block_id: match value.prev_block_id.as_ref() {
                Some(id) => Some(id.to_vec()?),
                None => None,
            },
        };

        Ok(ret)
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::protos::HotstuffTimeoutVote)]
pub(crate) struct HotstuffTimeoutVote {
    pub high_qc: Option<HotstuffBlockQC>,
    pub round: u64,
    pub voter: ObjectId,
    pub signature: Signature,
}

impl HotstuffTimeoutVote {
    pub async fn new(
        high_qc: Option<HotstuffBlockQC>,
        round: u64,
        local_id: ObjectId,
        signer: &RsaCPUObjectSigner,
    ) -> BuckyResult<Self> {
        let signature = signer
            .sign(
                Self::hash_content(high_qc.as_ref().map_or(0, |qc| qc.round), round).as_slice(),
                &SignatureSource::RefIndex(0),
            )
            .await?;

        Ok(Self {
            high_qc,
            round,
            voter: local_id,
            signature,
        })
    }

    fn hash(&self) -> HashValue {
        Self::hash_content(self.high_qc.as_ref().map_or(0, |qc| qc.round), self.round)
    }

    fn hash_content(high_qc_round: u64, round: u64) -> HashValue {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(high_qc_round.to_le_bytes());
        sha256.input(round.to_le_bytes());
        sha256.result().into()
    }
}

impl ProtobufTransform<crate::protos::HotstuffTimeoutVote> for HotstuffTimeoutVote {
    fn transform(value: crate::protos::HotstuffTimeoutVote) -> BuckyResult<Self> {
        let high_qc = if value.high_qc().len() == 0 {
            None
        } else {
            Some(HotstuffBlockQC::raw_decode(value.high_qc())?.0)
        };
        Ok(Self {
            voter: ObjectId::raw_decode(value.voter.as_slice())?.0,
            signature: Signature::raw_decode(value.signature.as_slice())?.0,
            round: value.round,
            high_qc,
        })
    }
}

impl ProtobufTransform<&HotstuffTimeoutVote> for crate::protos::HotstuffTimeoutVote {
    fn transform(value: &HotstuffTimeoutVote) -> BuckyResult<Self> {
        let ret = crate::protos::HotstuffTimeoutVote {
            high_qc: match value.high_qc.as_ref() {
                Some(qc) => Some(qc.to_vec()?),
                None => None,
            },
            round: value.round,
            voter: value.voter.to_vec()?,
            signature: value.signature.to_vec()?,
        };

        Ok(ret)
    }
}

#[cfg(test)]
mod test {}