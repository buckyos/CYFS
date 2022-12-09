use std::sync::atomic::{AtomicU8, Ordering};

use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;
use sha2::Digest;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::GroupConsensusBlockDescContent)]
pub struct GroupConsensusBlockDescContent {
    r_path_id: ObjectId,
    proposals_hash: HashValue,
    result_state_id: ObjectId,
    proposal_result_states_hash: HashValue,
    proposal_receiptes_hash: HashValue,
    version_seq: u64,
    meta_block_id: ObjectId,
    timestamp: u64,
    prev_block_id: Option<ObjectId>,
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

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::GroupConsensusBlockBodyContent)]
pub struct GroupConsensusBlockBodyContent {
    proposals: Vec<ObjectId>,
    proposal_result_states: Vec<ObjectId>,
    proposal_receiptes: Vec<ObjectId>,
}

impl BodyContent for GroupConsensusBlockBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type GroupConsensusBlockType =
    NamedObjType<GroupConsensusBlockDescContent, GroupConsensusBlockBodyContent>;
type GroupConsensusBlockBuilder =
    NamedObjectBuilder<GroupConsensusBlockDescContent, GroupConsensusBlockBodyContent>;

pub type GroupConsensusBlockId = NamedObjectId<GroupConsensusBlockType>;
pub struct GroupConsensusBlock(NamedObjectBase<GroupConsensusBlockType>, AtomicU8);

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

pub trait GroupConsensusBlockObject {
    fn create(
        r_path_id: ObjectId,
        proposals: Vec<ObjectId>,
        result_state_id: ObjectId,
        proposal_result_states: Vec<ObjectId>,
        proposal_receiptes: Vec<ObjectId>,
        version_seq: u64,
        meta_block_id: ObjectId,
        prev_block_id: Option<ObjectId>,
        owner: ObjectId,
    ) -> Self;
    fn check(&self) -> bool;
    fn r_path_id(&self) -> &ObjectId;
    fn proposals(&self) -> &Vec<ObjectId>;
    fn result_state_id(&self) -> &ObjectId;
    fn proposal_result_states(&self) -> &Vec<ObjectId>;
    fn proposal_receiptes(&self) -> &Vec<ObjectId>;
    fn version_seq(&self) -> u64;
    fn meta_block_id(&self) -> &ObjectId;
    fn prev_block_id(&self) -> &Option<ObjectId>;
    fn owner(&self) -> &ObjectId;
    fn named_object(&self) -> &NamedObjectBase<GroupConsensusBlockType>;
}

impl GroupConsensusBlockObject for GroupConsensusBlock {
    fn create(
        r_path_id: ObjectId,
        proposals: Vec<ObjectId>,
        result_state_id: ObjectId,
        proposal_result_states: Vec<ObjectId>,
        proposal_receiptes: Vec<ObjectId>,
        version_seq: u64,
        meta_block_id: ObjectId,
        prev_block_id: Option<ObjectId>,
        owner: ObjectId,
    ) -> Self {
        let desc = GroupConsensusBlockDescContent {
            r_path_id,
            proposals_hash: GroupConsensusBlockDescContent::hash_object_vec(proposals.as_slice()),
            result_state_id,
            proposal_result_states_hash: GroupConsensusBlockDescContent::hash_object_vec(
                proposal_result_states.as_slice(),
            ),
            proposal_receiptes_hash: GroupConsensusBlockDescContent::hash_object_vec(
                proposal_receiptes.as_slice(),
            ),
            version_seq,
            meta_block_id,
            timestamp: bucky_time_now(),
            prev_block_id,
        };

        let body = GroupConsensusBlockBodyContent {
            proposals,
            proposal_result_states,
            proposal_receiptes,
        };

        let block = GroupConsensusBlockBuilder::new(desc, body)
            .owner(owner)
            .build();

        Self(block, AtomicU8::new(BLOCK_CHECK_STATE_SUCC))
    }

    fn check(&self) -> bool {
        let state = self.1.load(Ordering::SeqCst);
        if state == BLOCK_CHECK_STATE_NONE {
            let desc = self.0.desc().content();
            let body = self.0.body().as_ref().unwrap().content();
            if GroupConsensusBlockDescContent::hash_object_vec(body.proposals.as_slice())
                != desc.proposals_hash
                || GroupConsensusBlockDescContent::hash_object_vec(
                    body.proposal_result_states.as_slice(),
                ) != desc.proposal_result_states_hash
                || GroupConsensusBlockDescContent::hash_object_vec(
                    body.proposal_receiptes.as_slice(),
                ) != desc.proposal_receiptes_hash
            {
                self.1.store(BLOCK_CHECK_STATE_FAIL, Ordering::SeqCst);
                false
            } else {
                self.1.store(BLOCK_CHECK_STATE_SUCC, Ordering::SeqCst);
                true
            }
        } else {
            state == BLOCK_CHECK_STATE_SUCC
        }
    }

    fn r_path_id(&self) -> &ObjectId {
        let desc = self.0.desc().content();
        &desc.r_path_id
    }

    fn proposals(&self) -> &Vec<ObjectId> {
        let body = self.0.body().as_ref().unwrap().content();
        &body.proposals
    }

    fn result_state_id(&self) -> &ObjectId {
        let desc = self.0.desc().content();
        &desc.result_state_id
    }

    fn proposal_result_states(&self) -> &Vec<ObjectId> {
        let body = self.0.body().as_ref().unwrap().content();
        &body.proposal_result_states
    }

    fn proposal_receiptes(&self) -> &Vec<ObjectId> {
        let body = self.0.body().as_ref().unwrap().content();
        &body.proposal_receiptes
    }

    fn version_seq(&self) -> u64 {
        let desc = self.0.desc().content();
        desc.version_seq
    }

    fn meta_block_id(&self) -> &ObjectId {
        let desc = self.0.desc().content();
        &desc.meta_block_id
    }

    fn prev_block_id(&self) -> &Option<ObjectId> {
        let desc = self.0.desc().content();
        &desc.prev_block_id
    }

    fn owner(&self) -> &ObjectId {
        let desc = self.0.desc();
        desc.owner().as_ref().unwrap()
    }

    fn named_object(&self) -> &NamedObjectBase<GroupConsensusBlockType> {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::{GroupConsensusBlock, GroupConsensusBlockObject};
    use cyfs_base::*;

    #[async_std::test]
    async fn create_group_rpath() {
        let secret1 = PrivateKey::generate_rsa(1024).unwrap();
        let secret2 = PrivateKey::generate_rsa(1024).unwrap();
        let people1 = People::new(None, vec![], secret1.public(), None, None, None).build();
        let people1_id = people1.desc().people_id();
        let people2 = People::new(None, vec![], secret2.public(), None, None, None).build();
        let _people2_id = people2.desc().people_id();

        let g1 = GroupConsensusBlock::create(
            people1_id.object_id().to_owned(),
            people1_id.object_id().to_owned(),
            people1_id.to_string(),
        );

        let buf = g1.to_vec().unwrap();
        let add2 = GroupConsensusBlock::clone_from_slice(&buf).unwrap();
        let any = AnyNamedObject::clone_from_slice(&buf).unwrap();
        assert_eq!(g1.desc().calculate_id(), add2.desc().calculate_id());
        assert_eq!(g1.desc().calculate_id(), any.calculate_id());
    }
}
