use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, HashValue, ObjectId, ObjectLink, ProtobufDecode,
    ProtobufEncode, ProtobufTransform, ProtobufTransformType, RawConvertTo, RawDecode,
    RawEncodePurpose, RsaCPUObjectSigner, Signature, SignatureSource, Signer,
};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, HotstuffBlockQC};
use sha2::Digest;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::codec::protos::HotstuffBlockQcVote)]
pub struct HotstuffBlockQCVote {
    pub block_id: ObjectId,
    pub prev_block_id: Option<ObjectId>,
    pub round: u64,
    pub voter: ObjectId,
    pub signature: Signature,
}

impl HotstuffBlockQCVote {
    pub async fn new(
        block: &GroupConsensusBlock,
        local_device_id: ObjectId,
        signer: &RsaCPUObjectSigner,
    ) -> BuckyResult<Self> {
        let block_id = block.block_id().object_id();
        let round = block.round();

        log::debug!(
            "[block vote] local: {:?}, vote hash {}, round: {}",
            local_device_id,
            block.block_id(),
            block.round()
        );

        let hash = Self::hash_content(block_id, block.prev_block_id(), round);

        log::debug!(
            "[block vote] local: {:?}, vote sign {}, round: {}",
            local_device_id,
            block.block_id(),
            block.round()
        );

        let signature = signer
            .sign(
                hash.as_slice(),
                &SignatureSource::Object(ObjectLink {
                    obj_id: local_device_id,
                    obj_owner: None,
                }),
            )
            .await?;

        Ok(Self {
            block_id: block_id.clone(),
            round,
            voter: local_device_id,
            signature,
            prev_block_id: block.prev_block_id().map(|id| id.clone()),
        })
    }

    pub fn hash(&self) -> HashValue {
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

impl ProtobufTransform<super::codec::protos::HotstuffBlockQcVote> for HotstuffBlockQCVote {
    fn transform(value: super::codec::protos::HotstuffBlockQcVote) -> BuckyResult<Self> {
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

impl ProtobufTransform<&HotstuffBlockQCVote> for super::codec::protos::HotstuffBlockQcVote {
    fn transform(value: &HotstuffBlockQCVote) -> BuckyResult<Self> {
        let ret = super::codec::protos::HotstuffBlockQcVote {
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
#[cyfs_protobuf_type(super::codec::protos::HotstuffTimeoutVote)]
pub struct HotstuffTimeoutVote {
    pub high_qc: Option<HotstuffBlockQC>,
    pub round: u64,
    pub voter: ObjectId,
    pub signature: Signature,
}

impl HotstuffTimeoutVote {
    pub async fn new(
        high_qc: Option<HotstuffBlockQC>,
        round: u64,
        local_device_id: ObjectId,
        signer: &RsaCPUObjectSigner,
    ) -> BuckyResult<Self> {
        let signature = signer
            .sign(
                Self::hash_content(high_qc.as_ref().map_or(0, |qc| qc.round), round).as_slice(),
                &SignatureSource::Object(ObjectLink {
                    obj_id: local_device_id,
                    obj_owner: None,
                }),
            )
            .await?;

        Ok(Self {
            high_qc,
            round,
            voter: local_device_id,
            signature,
        })
    }

    pub fn hash(&self) -> HashValue {
        Self::hash_content(self.high_qc.as_ref().map_or(0, |qc| qc.round), self.round)
    }

    pub fn hash_content(high_qc_round: u64, round: u64) -> HashValue {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(high_qc_round.to_le_bytes());
        sha256.input(round.to_le_bytes());
        sha256.result().into()
    }
}

impl ProtobufTransform<super::codec::protos::HotstuffTimeoutVote> for HotstuffTimeoutVote {
    fn transform(value: super::codec::protos::HotstuffTimeoutVote) -> BuckyResult<Self> {
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

impl ProtobufTransform<&HotstuffTimeoutVote> for super::codec::protos::HotstuffTimeoutVote {
    fn transform(value: &HotstuffTimeoutVote) -> BuckyResult<Self> {
        let ret = super::codec::protos::HotstuffTimeoutVote {
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
