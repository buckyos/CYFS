use std::mem;

use async_std::task;
use cyfs_base::*;
use cyfs_core::{
    GroupProposal, GroupProposalBuilder, GroupProposalDescContent, GroupProposalObject,
    GroupPropsalDecideParam, GroupRPath, GroupUpdateGroupPropsalParam,
};

use crate::{GROUP_METHOD_UPDATE, STATEPATH_GROUP_DEC_ID, STATEPATH_GROUP_DEC_RPATH};

pub struct GroupUpdateProposal {
    proposal: GroupProposal,
    target_dec_id: Vec<ObjectId>,
    from_blob_id: Option<ObjectId>,
    to_group: Group,
}

impl GroupUpdateProposal {
    pub fn create(
        from_group_blob_id: Option<ObjectId>,
        to_group: Group,
        owner: ObjectId,
        target_dec_id: Vec<ObjectId>,
        meta_block_id: Option<ObjectId>,
        effective_begining: Option<u64>,
        effective_ending: Option<u64>,
    ) -> GroupUpdateProposalBuilder {
        GroupUpdateProposalBuilder::create(
            from_group_blob_id,
            to_group,
            owner,
            target_dec_id,
            meta_block_id,
            effective_begining,
            effective_ending,
        )
    }

    pub fn create_new_group(
        to_group: Group,
        owner: ObjectId,
        target_dec_id: Vec<ObjectId>,
        meta_block_id: Option<ObjectId>,
        effective_begining: Option<u64>,
        effective_ending: Option<u64>,
    ) -> GroupUpdateProposalBuilder {
        GroupUpdateProposalBuilder::create(
            None,
            to_group,
            owner,
            target_dec_id,
            meta_block_id,
            effective_begining,
            effective_ending,
        )
    }

    pub fn base(&self) -> &GroupProposal {
        &self.proposal
    }

    pub fn target_dec_id(&self) -> &[ObjectId] {
        self.target_dec_id.as_slice()
    }

    pub fn from_blob_id(&self) -> &Option<ObjectId> {
        &self.from_blob_id
    }

    pub fn to_group(&self) -> &Group {
        &self.to_group
    }

    pub async fn decide(
        &mut self,
        member_id: ObjectId,
        decide: Vec<u8>,
        private_key: &PrivateKey,
    ) -> BuckyResult<GroupPropsalDecideParam> {
        self.proposal.decide(member_id, decide, private_key).await
    }

    async fn verify_and_merge_decide(
        &mut self,
        decide: &GroupPropsalDecideParam,
        member_id: ObjectId,
        public_key: &PublicKey,
    ) -> BuckyResult<()> {
        self.proposal
            .verify_and_merge_decide(decide, member_id, public_key)
            .await
    }

    pub fn group_update_rpath(group_id: ObjectId) -> GroupRPath {
        GroupRPath::new(
            group_id,
            *STATEPATH_GROUP_DEC_ID,
            STATEPATH_GROUP_DEC_RPATH.to_string(),
        )
    }
}

impl TryFrom<GroupProposal> for GroupUpdateProposal {
    type Error = BuckyError;

    fn try_from(value: GroupProposal) -> Result<Self, Self::Error> {
        if value.params().is_none() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidFormat,
                "param for GroupUpdateProposal shoud not None",
            ));
        }

        let (param, remain) =
            GroupUpdateGroupPropsalParam::raw_decode(value.params().as_ref().unwrap().as_slice())?;
        assert_eq!(remain.len(), 0);

        if value.payload().is_none() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidFormat,
                "payload for GroupUpdateProposal shoud not None",
            ));
        }

        let payload = value.payload().as_ref().unwrap();
        let (group_blob, remain) = ChunkMeta::raw_decode(payload.as_slice()).unwrap();
        assert_eq!(remain.len(), 0);

        let to_group = Group::try_from(&group_blob)?;

        let to_blob_id =
            task::block_on(async { group_blob.to_chunk().await.unwrap().calculate_id() });
        if &to_blob_id.object_id() != param.to_blob_id() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidFormat,
                "the chunk in GroupUpdateProposal.body is not match with the to_blob_id in desc.param",
            ));
        }

        let ret = Self {
            proposal: value,
            target_dec_id: Vec::from(param.target_dec_id()),
            from_blob_id: param.from_blob_id().clone(),
            to_group,
        };

        Ok(ret)
    }
}

pub struct GroupUpdateProposalBuilder {
    proposal: GroupProposalBuilder,
    target_dec_id: Vec<ObjectId>,
    from_blob_id: Option<ObjectId>,
    to_group: Option<Group>,
}

impl GroupUpdateProposalBuilder {
    pub fn create(
        from_group_blob_id: Option<ObjectId>,
        to_group: Group,
        owner: ObjectId,
        target_dec_id: Vec<ObjectId>,
        meta_block_id: Option<ObjectId>,
        effective_begining: Option<u64>,
        effective_ending: Option<u64>,
    ) -> Self {
        let group_blob = ChunkMeta::from(&to_group);
        let group_blob_vec = {
            let len = group_blob.raw_measure(&None).unwrap();
            let mut buf = vec![0u8; len];
            let remain = group_blob.raw_encode(buf.as_mut_slice(), &None).unwrap();
            assert_eq!(remain.len(), 0);
            buf
        };

        let to_blob_id =
            task::block_on(async { group_blob.to_chunk().await.unwrap().calculate_id() });

        let param =
            GroupUpdateGroupPropsalParam::new(target_dec_id.clone(), None, to_blob_id.object_id());
        let param_vec = {
            let len = param.raw_measure(&None).unwrap();
            let mut buf = vec![0u8; len];
            let remain = param.raw_encode(buf.as_mut_slice(), &None).unwrap();
            assert_eq!(remain.len(), 0);
            buf
        };

        let group_id = to_group.desc().group_id();
        let update_rpath = GroupUpdateProposal::group_update_rpath(group_id.object_id().clone());

        let proposal = GroupProposal::create(
            update_rpath,
            GROUP_METHOD_UPDATE.to_string(),
            Some(param_vec),
            Some(group_blob_vec),
            None,
            owner,
            meta_block_id,
            effective_begining,
            effective_ending,
        );

        Self {
            proposal,
            target_dec_id,
            from_blob_id: from_group_blob_id,
            to_group: Some(to_group),
        }
    }

    pub fn create_new_group(
        to_group: Group,
        owner: ObjectId,
        target_dec_id: Vec<ObjectId>,
        meta_block_id: Option<ObjectId>,
        effective_begining: Option<u64>,
        effective_ending: Option<u64>,
    ) -> Self {
        Self::create(
            None,
            to_group,
            owner,
            target_dec_id,
            meta_block_id,
            effective_begining,
            effective_ending,
        )
    }

    pub fn desc_builder(&mut self) -> &mut NamedObjectDescBuilder<GroupProposalDescContent> {
        self.proposal.mut_desc_builder()
    }

    pub fn build(mut self) -> GroupUpdateProposal {
        let mut target_dec_id: Vec<ObjectId> = vec![];
        let mut from_blob_id: Option<ObjectId> = None;
        let to_group = self.to_group.take().unwrap();
        mem::swap(&mut target_dec_id, &mut self.target_dec_id);
        mem::swap(&mut from_blob_id, &mut self.from_blob_id);

        GroupUpdateProposal {
            proposal: self.proposal.build(),
            target_dec_id,
            from_blob_id,
            to_group,
        }
    }
}
