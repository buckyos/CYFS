
use std::{
    convert::TryFrom, 
    time::Duration, 
};
use async_std::{
    io::prelude::*
};

use sha2::Digest;
use cyfs_base::*;
use cyfs_bdt::*;
use cyfs_lib::*;
use crate::{
    protos, 
    obj_id, 
    data_source::*, 
    contracts::dsg_dec_id
};


#[derive(Clone, Debug)]
pub struct DsgChallengeSample {
    pub chunk_index: u16,
    pub offset_in_chunk: u64,
    pub sample_len: u16,
}

impl TryFrom<&DsgChallengeSample> for protos::ChallengeSample {
    type Error = BuckyError;

    fn try_from(rust: &DsgChallengeSample) -> BuckyResult<Self> {
        let mut proto = protos::ChallengeSample::new();
        proto.set_chunk_index(rust.chunk_index as u32);
        proto.set_offset_in_chunk(rust.offset_in_chunk);
        proto.set_sample_len(rust.sample_len as u32);
        Ok(proto)
    }
}

impl TryFrom<protos::ChallengeSample> for DsgChallengeSample {
    type Error = BuckyError;

    fn try_from(proto: protos::ChallengeSample) -> BuckyResult<Self> {
        Ok(Self {
            chunk_index: proto.get_chunk_index() as u16,
            offset_in_chunk: proto.get_offset_in_chunk(),
            sample_len: proto.get_sample_len() as u16,
        })
    }
}

impl_default_protobuf_raw_codec!(DsgChallengeSample, protos::ChallengeSample);

#[derive(Clone)]
pub struct DsgChallengeDesc {
    contract_id: ObjectId,
    contract_state: ObjectId,
    samples: Vec<DsgChallengeSample>,
}

impl TryFrom<&DsgChallengeDesc> for protos::ChallengeDesc {
    type Error = BuckyError;

    fn try_from(rust: &DsgChallengeDesc) -> BuckyResult<Self> {
        let mut proto = protos::ChallengeDesc::new();
        proto.set_contract_id(rust.contract_id.to_vec()?);
        proto.set_contract_state(rust.contract_state.to_vec()?);
        proto.set_samples(ProtobufCodecHelper::encode_nested_list(&rust.samples)?);
        Ok(proto)
    }
}

impl TryFrom<protos::ChallengeDesc> for DsgChallengeDesc {
    type Error = BuckyError;

    fn try_from(mut proto: protos::ChallengeDesc) -> BuckyResult<Self> {
        Ok(Self {
            contract_id: ProtobufCodecHelper::decode_buf(proto.take_contract_id())?,
            contract_state: ProtobufCodecHelper::decode_buf(proto.take_contract_state())?,
            samples: ProtobufCodecHelper::decode_nested_list(proto.take_samples())?,
        })
    }
}

impl_default_protobuf_raw_codec!(DsgChallengeDesc, protos::ChallengeDesc);

impl DescContent for DsgChallengeDesc {
    fn obj_type() -> u16 {
        obj_id::CHALLENGE_OBJECT_TYPE
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct DsgChallengeBody {}

impl BodyContent for DsgChallengeBody {}

pub type DsgChallengeObjectType = NamedObjType<DsgChallengeDesc, DsgChallengeBody>;
pub type DsgChallengeObject = NamedObjectBase<DsgChallengeObjectType>;

#[derive(Copy, Clone)]
pub struct DsgChallengeObjectRef<'a> {
    obj: &'a DsgChallengeObject,
}

impl<'a> std::fmt::Display for DsgChallengeObjectRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DsgChallengeObject{{id={}, contract={}, state={}, create_at={}, expire_at={}, samples={:?}}}", 
            self.id(), self.contract_id(), self.contract_state(), self.create_at(), self.expire_at(), self.samples())
    }
}

impl<'a> AsRef<DsgChallengeObject> for DsgChallengeObjectRef<'a> {
    fn as_ref(&self) -> &DsgChallengeObject {
        self.obj
    }
}

impl<'a> From<&'a DsgChallengeObject> for DsgChallengeObjectRef<'a> {
    fn from(obj: &'a DsgChallengeObject) -> Self {
        Self { obj }
    }
}

#[derive(Debug)]
pub struct DsgChallengeOptions {
    pub sample_count: u16,
    pub sample_len: u16,
    pub live_time: Duration,
}

impl<'a> DsgChallengeObjectRef<'a> {
    pub fn id(&self) -> ObjectId {
        self.obj.desc().object_id()
    }

    pub fn owner(&self) -> &ObjectId {
        self.obj.desc().owner().as_ref().unwrap()
    }

    pub fn create_at(&self) -> u64 {
        self.obj.desc().create_time()
    }

    pub fn expire_at(&self) -> u64 {
        self.obj.desc().expired_time().unwrap()
    }

    pub fn contract_id(&self) -> &ObjectId {
        &self.obj.desc().content().contract_id
    }

    pub fn contract_state(&self) -> &ObjectId {
        &self.obj.desc().content().contract_state
    }

    pub fn samples(&self) -> &Vec<DsgChallengeSample> {
        &self.obj.desc().content().samples
    }

    pub fn new<'b>(
        owner: ObjectId,
        contract_id: ObjectId,
        contract_state: ObjectId,
        chunks: &Vec<ChunkId>,
        options: &DsgChallengeOptions,
    ) -> DsgChallengeObject {
        let chunk_list = ChunkListDesc::from_chunks(chunks);
        let mut samples = vec![];
        for start in (0..options.sample_count)
            .into_iter()
            .map(|_| (rand::random::<f64>() * chunk_list.total_len() as f64) as u64)
        {
            let new_samples = chunk_list
                .range_of(start..start + options.sample_len as u64)
                .into_iter()
                .map(|(index, range)| DsgChallengeSample {
                    chunk_index: index as u16,
                    offset_in_chunk: range.start,
                    sample_len: (range.end - range.start) as u16,
                });
            samples.append(&mut new_samples.collect());
        }
        let desc = DsgChallengeDesc {
            contract_id,
            contract_state: contract_state.clone(),
            samples,
        };
        let now = bucky_time_now();
        let challenge = NamedObjectBuilder::new(desc, DsgChallengeBody {})
            .dec_id(dsg_dec_id())
            .ref_objects(vec![ObjectLink {
                obj_id: contract_state,
                obj_owner: None,
            }])
            .create_time(now)
            .expired_time(now + options.live_time.as_micros() as u64)
            .owner(owner)
            .build();
        challenge
    }
}

#[derive(Clone)]
pub struct DsgProofDesc {
    challenge: ObjectId,
    proof: HashValue,
}

impl TryFrom<&DsgProofDesc> for protos::ProofDesc {
    type Error = BuckyError;

    fn try_from(rust: &DsgProofDesc) -> BuckyResult<Self> {
        let mut proto = protos::ProofDesc::new();
        proto.set_challenge(rust.challenge.to_vec()?);
        proto.set_proof(rust.proof.to_vec()?);
        Ok(proto)
    }
}

impl TryFrom<protos::ProofDesc> for DsgProofDesc {
    type Error = BuckyError;

    fn try_from(mut proto: protos::ProofDesc) -> BuckyResult<Self> {
        Ok(Self {
            challenge: ProtobufCodecHelper::decode_buf(proto.take_challenge())?,
            proof: ProtobufCodecHelper::decode_buf(proto.take_proof())?,
        })
    }
}

impl_default_protobuf_raw_codec!(DsgProofDesc, protos::ProofDesc);

impl DescContent for DsgProofDesc {
    fn obj_type() -> u16 {
        obj_id::PROOF_OBJECT_TYPE
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct DsgProofBody {}

impl BodyContent for DsgProofBody {}

pub type DsgProofObjectType = NamedObjType<DsgProofDesc, DsgProofBody>;
pub type DsgProofObject = NamedObjectBase<DsgProofObjectType>;

#[derive(Copy, Clone)]
pub struct DsgProofObjectRef<'a> {
    obj: &'a DsgProofObject,
}

impl<'a> std::fmt::Display for DsgProofObjectRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DsgProofObject{{id={}, challenge={}, proof={}}}",
            self.id(),
            self.challenge(),
            self.obj.desc().content().proof
        )
    }
}

impl<'a> AsRef<DsgProofObject> for DsgProofObjectRef<'a> {
    fn as_ref(&self) -> &DsgProofObject {
        self.obj
    }
}

impl<'a> From<&'a DsgProofObject> for DsgProofObjectRef<'a> {
    fn from(obj: &'a DsgProofObject) -> Self {
        Self { obj }
    }
}

impl<'a> DsgProofObjectRef<'a> {
    pub fn id(&self) -> ObjectId {
        self.obj.desc().object_id()
    }

    pub fn challenge(&self) -> &ObjectId {
        &self.obj.desc().content().challenge
    }

    // 生成proof
    pub async fn proove<'b>(
        challenge_ref: DsgChallengeObjectRef<'b>,
        chunks: &Vec<ChunkId>,
        reader: Box<dyn ChunkReader>,
    ) -> BuckyResult<DsgProofObject> {
        let mut hasher = sha2::Sha256::new();
        hasher.input(&challenge_ref.id().to_vec()?[..]);
        for sample in challenge_ref.samples() {
            let mut r = reader
                .read_ext(
                    &chunks[sample.chunk_index as usize],
                    vec![
                        sample.offset_in_chunk..(sample.offset_in_chunk + sample.sample_len as u64),
                    ],
                )
                .await?;
            let mut buf = vec![0u8; sample.sample_len as usize];
            let _ = r.read(buf.as_mut_slice()).await?;
            hasher.input(&buf[..]);
        }

        let desc = DsgProofDesc {
            challenge: challenge_ref.id(),
            proof: hasher.result().into(),
        };
        let proof = NamedObjectBuilder::new(desc, DsgProofBody {})
            .dec_id(dsg_dec_id())
            .ref_objects(vec![ObjectLink {
                obj_id: challenge_ref.id(),
                obj_owner: None,
            }])
            .build();
        Ok(proof)
    }

    // 校验 proof
    pub async fn verify<'b>(
        &self, 
        stack: &SharedCyfsStack, 
        challenge_ref: DsgChallengeObjectRef<'b>,
        merged: ChunkListDesc, 
        sources: ChunkListDesc, 
        stub: DsgDataSourceStubObjectRef<'b>,
        reader: Box<dyn ChunkReader>,
    ) -> BuckyResult<bool> {
        let mut hasher = sha2::Sha256::new();
        hasher.input(&challenge_ref.id().to_vec()?[..]);
        for sample in challenge_ref.samples() {
            let mut r = stub.read_sample(stack, &reader, merged.clone(), sources.clone(), sample).await?;
            let mut buf = vec![0u8; sample.sample_len as usize];
            let _ = r.read(buf.as_mut_slice()).await?;
            hasher.input(&buf[..]);
        }
        let proof = hasher.result().into();
        Ok(self.as_ref().desc().content().proof == proof)
    }
}

