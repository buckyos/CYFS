use std::mem;

use cyfs_base::*;
use cyfs_core::{
    GroupProposal, GroupProposalBuilder, GroupProposalDescContent, GroupProposalObject,
    GroupPropsalDecideParam, GroupRPath,
};

use crate::GROUP_METHOD_DECIDE;

pub struct GroupDecideProposal {
    proposal: GroupProposal,
    decides: Vec<GroupPropsalDecideParam>,
}

impl GroupDecideProposal {
    pub fn create(
        rpath: GroupRPath,
        decides: Vec<GroupPropsalDecideParam>,
        owner_id: ObjectId,
    ) -> GroupDecideProposalBuilder {
        GroupDecideProposalBuilder::create(rpath, decides, owner_id)
    }

    pub fn base(&self) -> &GroupProposal {
        &self.proposal
    }

    pub fn decides(&self) -> &[GroupPropsalDecideParam] {
        self.decides.as_slice()
    }
}

impl TryFrom<GroupProposal> for GroupDecideProposal {
    type Error = BuckyError;

    fn try_from(value: GroupProposal) -> Result<Self, Self::Error> {
        if value.params().is_none() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidFormat,
                "param for GroupDecideProposal shoud not None",
            ));
        }

        let (decides, remain) = Vec::<GroupPropsalDecideParam>::raw_decode(
            value.params().as_ref().unwrap().as_slice(),
        )?;
        assert_eq!(remain.len(), 0);

        let ret = GroupDecideProposal {
            proposal: value,
            decides,
        };
        Ok(ret)
    }
}

pub struct GroupDecideProposalBuilder {
    proposal: GroupProposalBuilder,
    decides: Vec<GroupPropsalDecideParam>,
}

impl GroupDecideProposalBuilder {
    pub fn create(
        rpath: GroupRPath,
        decides: Vec<GroupPropsalDecideParam>,
        owner: ObjectId,
    ) -> Self {
        let param_vec = {
            let len = decides.raw_measure(&None).unwrap();
            let mut buf = vec![0u8; len];
            let remain = decides.raw_encode(buf.as_mut_slice(), &None).unwrap();
            assert_eq!(remain.len(), 0);
            buf
        };

        let proposal = GroupProposal::create(
            rpath,
            GROUP_METHOD_DECIDE.to_string(),
            Some(param_vec),
            None,
            None,
            owner,
            None,
            None,
            None,
        );

        Self { proposal, decides }
    }

    pub fn desc_builder(&mut self) -> &mut NamedObjectDescBuilder<GroupProposalDescContent> {
        self.proposal.mut_desc_builder()
    }

    pub fn build(mut self) -> GroupDecideProposal {
        let mut decides = vec![];
        mem::swap(&mut decides, &mut self.decides);

        GroupDecideProposal {
            proposal: self.proposal.build(),
            decides,
        }
    }
}
