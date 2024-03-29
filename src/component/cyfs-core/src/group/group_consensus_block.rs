use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

use crate::{CoreObjectType, GroupRPath};
use cyfs_base::*;
use sha2::Digest;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::GroupConsensusBlockDescContent)]
pub struct GroupConsensusBlockDescContent {
    rpath: GroupRPath,
    body_hash: HashValue,
    result_state_id: Option<ObjectId>,
    height: u64,
    meta_block_id: ObjectId,
    round: u64,
    group_shell_id: ObjectId,
}

impl DescContent for GroupConsensusBlockDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::GroupConsensusBlock as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn debug_info() -> String {
        String::from("GroupConsensusBlockDescContent")
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

impl GroupConsensusBlockDescContent {
    pub fn rpath(&self) -> &GroupRPath {
        &self.rpath
    }

    pub fn result_state_id(&self) -> &Option<ObjectId> {
        &self.result_state_id
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn round(&self) -> u64 {
        self.round
    }

    pub fn group_shell_id(&self) -> &ObjectId {
        &self.group_shell_id
    }
}

#[derive(Clone, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::hotstuff_block_qc::VoteSignature)]
pub struct HotstuffBlockQCSign {
    pub voter: ObjectId,
    pub signature: Signature,
}

impl ProtobufTransform<crate::codec::protos::hotstuff_block_qc::VoteSignature>
    for HotstuffBlockQCSign
{
    fn transform(
        value: crate::codec::protos::hotstuff_block_qc::VoteSignature,
    ) -> BuckyResult<Self> {
        Ok(Self {
            voter: ObjectId::raw_decode(value.voter.as_slice())?.0,
            signature: Signature::raw_decode(value.signature.as_slice())?.0,
        })
    }
}

impl ProtobufTransform<&HotstuffBlockQCSign>
    for crate::codec::protos::hotstuff_block_qc::VoteSignature
{
    fn transform(value: &HotstuffBlockQCSign) -> BuckyResult<Self> {
        Ok(Self {
            voter: value.voter.to_vec()?,
            signature: value.signature.to_vec()?,
        })
    }
}

#[derive(Default, Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::HotstuffBlockQc)]
pub struct HotstuffBlockQC {
    pub block_id: ObjectId,
    pub prev_block_id: Option<ObjectId>,
    pub round: u64,
    pub votes: Vec<HotstuffBlockQCSign>,
}

#[derive(Clone, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::hotstuff_timeout::VoteSignature)]
pub struct HotstuffTimeoutSign {
    pub voter: ObjectId,
    pub high_qc_round: u64,
    pub signature: Signature,
}

impl ProtobufTransform<crate::codec::protos::hotstuff_timeout::VoteSignature>
    for HotstuffTimeoutSign
{
    fn transform(
        value: crate::codec::protos::hotstuff_timeout::VoteSignature,
    ) -> BuckyResult<Self> {
        Ok(Self {
            voter: ObjectId::raw_decode(value.voter.as_slice())?.0,
            signature: Signature::raw_decode(value.signature.as_slice())?.0,
            high_qc_round: value.high_qc_round,
        })
    }
}

impl ProtobufTransform<&HotstuffTimeoutSign>
    for crate::codec::protos::hotstuff_timeout::VoteSignature
{
    fn transform(value: &HotstuffTimeoutSign) -> BuckyResult<Self> {
        Ok(Self {
            voter: value.voter.to_vec()?,
            signature: value.signature.to_vec()?,
            high_qc_round: value.high_qc_round,
        })
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::HotstuffTimeout)]
pub struct HotstuffTimeout {
    pub round: u64,
    pub votes: Vec<HotstuffTimeoutSign>,
    pub group_shell_id: Option<ObjectId>,
}

#[derive(Clone, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::group_consensus_block_body_content::Proposal)]
pub struct GroupConsensusBlockProposal {
    pub proposal: ObjectId,
    pub result_state: Option<ObjectId>,
    pub receipt: Option<Vec<u8>>,
    pub context: Option<Vec<u8>>,
}

impl ProtobufTransform<crate::codec::protos::group_consensus_block_body_content::Proposal>
    for GroupConsensusBlockProposal
{
    fn transform(
        mut value: crate::codec::protos::group_consensus_block_body_content::Proposal,
    ) -> BuckyResult<Self> {
        let result_state = match value.proposal_result_state {
            Some(state_id) => Some(ObjectId::raw_decode(state_id.as_slice())?.0),
            None => None,
        };

        Ok(Self {
            proposal: ObjectId::raw_decode(value.proposal_id.as_slice())?.0,
            result_state,
            receipt: value.proposal_receipt.take(),
            context: value.context.take(),
        })
    }
}

impl ProtobufTransform<&GroupConsensusBlockProposal>
    for crate::codec::protos::group_consensus_block_body_content::Proposal
{
    fn transform(value: &GroupConsensusBlockProposal) -> BuckyResult<Self> {
        Ok(Self {
            proposal_id: value.proposal.to_vec()?,
            proposal_result_state: value.result_state.map(|id| id.to_vec().unwrap()),
            proposal_receipt: value.receipt.clone(),
            context: value.context.clone(),
        })
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::GroupConsensusBlockBodyContent)]
pub struct GroupConsensusBlockBodyContent {
    proposals: Vec<GroupConsensusBlockProposal>,
    qc: Option<HotstuffBlockQC>,
    tc: Option<HotstuffTimeout>,
}

impl BodyContent for GroupConsensusBlockBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

pub type GroupConsensusBlockDesc = NamedObjectDesc<GroupConsensusBlockDescContent>;
type GroupConsensusBlockType =
    NamedObjType<GroupConsensusBlockDescContent, GroupConsensusBlockBodyContent>;
type GroupConsensusBlockBuilder =
    NamedObjectBuilder<GroupConsensusBlockDescContent, GroupConsensusBlockBodyContent>;

pub type GroupConsensusBlockId = NamedObjectId<GroupConsensusBlockType>;
#[derive(Clone)]
pub struct GroupConsensusBlock(
    NamedObjectBase<GroupConsensusBlockType>,
    Arc<(AtomicU8, GroupConsensusBlockId)>,
);

const BLOCK_CHECK_STATE_NONE: u8 = 0;
const BLOCK_CHECK_STATE_SUCC: u8 = 1;
const BLOCK_CHECK_STATE_FAIL: u8 = 2;

impl GroupConsensusBlockDescContent {
    fn hash_object_vec(object_ids: &[ObjectId]) -> HashValue {
        let mut sha256 = sha2::Sha256::new();
        for id in object_ids {
            sha256.input(id.as_slice());
        }

        sha256.result().into()
    }
}

impl GroupConsensusBlockBodyContent {
    fn hash(&self) -> HashValue {
        let buf = self.to_vec().unwrap();
        let mut sha256 = sha2::Sha256::new();
        sha256.input(buf.as_slice());
        sha256.result().into()
    }
}

pub trait GroupConsensusBlockObject {
    fn create(
        rpath: GroupRPath,
        proposals: Vec<GroupConsensusBlockProposal>,
        result_state_id: Option<ObjectId>,
        height: u64,
        meta_block_id: ObjectId,
        round: u64,
        group_shell_id: ObjectId,
        qc: Option<HotstuffBlockQC>,
        tc: Option<HotstuffTimeout>,
        owner: ObjectId,
    ) -> Self;
    fn check(&self) -> bool;
    fn rpath(&self) -> &GroupRPath;
    fn proposals(&self) -> &Vec<GroupConsensusBlockProposal>;
    fn result_state_id(&self) -> &Option<ObjectId>;
    fn height(&self) -> u64;
    fn block_id(&self) -> &GroupConsensusBlockId;
    fn meta_block_id(&self) -> &ObjectId;
    fn prev_block_id(&self) -> Option<&ObjectId>;
    fn owner(&self) -> &ObjectId;
    fn named_object(&self) -> &NamedObjectBase<GroupConsensusBlockType>;
    fn named_object_mut(&mut self) -> &mut NamedObjectBase<GroupConsensusBlockType>;
    fn round(&self) -> u64;
    fn group_shell_id(&self) -> &ObjectId;
    fn qc(&self) -> &Option<HotstuffBlockQC>;
    fn tc(&self) -> &Option<HotstuffTimeout>;
}

impl GroupConsensusBlockObject for GroupConsensusBlock {
    fn create(
        rpath: GroupRPath,
        proposals: Vec<GroupConsensusBlockProposal>,
        result_state_id: Option<ObjectId>,
        height: u64,
        meta_block_id: ObjectId,
        round: u64,
        group_shell_id: ObjectId,
        qc: Option<HotstuffBlockQC>,
        mut tc: Option<HotstuffTimeout>,
        owner: ObjectId,
    ) -> Self {
        if let Some(tc) = tc.as_mut() {
            if tc.group_shell_id.as_ref() == Some(&group_shell_id) {
                tc.group_shell_id = None;
            }
        }

        let body = GroupConsensusBlockBodyContent { proposals, qc, tc };

        let desc = GroupConsensusBlockDescContent {
            rpath,
            result_state_id,

            height,
            meta_block_id,
            body_hash: body.hash(),
            round,
            group_shell_id,
        };

        let block = GroupConsensusBlockBuilder::new(desc, body)
            .owner(owner)
            .create_time(bucky_time_now())
            .build();

        let block_id = GroupConsensusBlockId::try_from(block.desc().object_id()).unwrap();
        Self(
            block,
            Arc::new((AtomicU8::new(BLOCK_CHECK_STATE_SUCC), block_id)),
        )
    }

    fn check(&self) -> bool {
        let state = self.1 .0.load(Ordering::SeqCst);
        if state == BLOCK_CHECK_STATE_NONE {
            let desc = self.0.desc().content();
            let body = self.0.body().as_ref().unwrap().content();

            let is_result_state_match = body
                .proposals
                .last()
                .map_or(true, |p| p.result_state == desc.result_state_id);

            if is_result_state_match
                && self.0.desc().owner().is_some()
                && body.hash() == desc.body_hash
            {
                self.1 .0.store(BLOCK_CHECK_STATE_SUCC, Ordering::SeqCst);
                true
            } else {
                self.1 .0.store(BLOCK_CHECK_STATE_FAIL, Ordering::SeqCst);
                false
            }
        } else {
            state == BLOCK_CHECK_STATE_SUCC
        }
    }

    fn rpath(&self) -> &GroupRPath {
        let desc = self.0.desc().content();
        &desc.rpath
    }

    fn proposals(&self) -> &Vec<GroupConsensusBlockProposal> {
        let body = self.0.body().as_ref().unwrap().content();
        &body.proposals
    }

    fn result_state_id(&self) -> &Option<ObjectId> {
        let desc = self.0.desc().content();
        &desc.result_state_id
    }

    fn height(&self) -> u64 {
        let desc = self.0.desc().content();
        desc.height
    }

    fn block_id(&self) -> &GroupConsensusBlockId {
        &self.1 .1
    }

    fn meta_block_id(&self) -> &ObjectId {
        let desc = self.0.desc().content();
        &desc.meta_block_id
    }

    fn prev_block_id(&self) -> Option<&ObjectId> {
        let body = self.0.body().as_ref().unwrap().content();
        body.qc.as_ref().map(|qc| &qc.block_id)
    }

    fn owner(&self) -> &ObjectId {
        let desc = self.0.desc();
        desc.owner().as_ref().unwrap()
    }

    fn named_object(&self) -> &NamedObjectBase<GroupConsensusBlockType> {
        &self.0
    }

    fn named_object_mut(&mut self) -> &mut NamedObjectBase<GroupConsensusBlockType> {
        &mut self.0
    }

    fn round(&self) -> u64 {
        let desc = self.0.desc().content();
        desc.round
    }

    fn group_shell_id(&self) -> &ObjectId {
        let desc = self.0.desc().content();
        &desc.group_shell_id
    }

    fn qc(&self) -> &Option<HotstuffBlockQC> {
        let body = self.0.body().as_ref().unwrap().content();
        &body.qc
    }

    fn tc(&self) -> &Option<HotstuffTimeout> {
        let body = self.0.body().as_ref().unwrap().content();
        &body.tc
    }
}

impl RawEncode for GroupConsensusBlock {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        self.0.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        self.0.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for GroupConsensusBlock {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (obj, remain) = NamedObjectBase::<GroupConsensusBlockType>::raw_decode(buf)?;
        let block_id = GroupConsensusBlockId::try_from(obj.desc().object_id()).unwrap();
        Ok((
            Self(
                obj,
                Arc::new((AtomicU8::new(BLOCK_CHECK_STATE_NONE), block_id)),
            ),
            remain,
        ))
    }
}

#[cfg(test)]
mod test {
    use super::{GroupConsensusBlock, GroupConsensusBlockObject};
    use cyfs_base::*;

    #[async_std::test]
    async fn create_group_rpath() {
        // let secret1 = PrivateKey::generate_rsa(1024).unwrap();
        // let secret2 = PrivateKey::generate_rsa(1024).unwrap();
        // let people1 = People::new(None, vec![], secret1.public(), None, None, None).build();
        // let people1_id = people1.desc().people_id();
        // let people2 = People::new(None, vec![], secret2.public(), None, None, None).build();
        // let _people2_id = people2.desc().people_id();

        // let g1 = GroupConsensusBlock::create(
        //     people1_id.object_id().to_owned(),
        //     people1_id.object_id().to_owned(),
        //     people1_id.to_string(),
        // );

        // let buf = g1.to_vec().unwrap();
        // let add2 = GroupConsensusBlock::clone_from_slice(&buf).unwrap();
        // let any = AnyNamedObject::clone_from_slice(&buf).unwrap();
        // assert_eq!(g1.desc().calculate_id(), add2.desc().calculate_id());
        // assert_eq!(g1.desc().calculate_id(), any.calculate_id());
    }
}
