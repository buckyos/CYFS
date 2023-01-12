use cyfs_base::{BuckyResult, Group, ObjectId, RawDecode};
use cyfs_core::{GroupConsensusBlock, GroupProposal};
use cyfs_lib::NONObjectInfo;

#[derive(Clone)]
pub struct NonDriver {}

impl NonDriver {
    pub async fn get_object(
        &self,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<NONObjectInfo> {
        unimplemented!()
    }

    pub async fn post_object(&self, obj: NONObjectInfo, to: &ObjectId) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn broadcast(&self, obj: NONObjectInfo, to: &[ObjectId]) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn get_block(
        &self,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<GroupConsensusBlock> {
        let obj = self.get_object(object_id, from).await?;
        let (block, remain) = GroupConsensusBlock::raw_decode(obj.object_raw.as_slice())?;
        assert_eq!(remain.len(), 0);
        Ok(block)
    }

    pub async fn get_proposal(
        &self,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<GroupProposal> {
        let obj = self.get_object(object_id, from).await?;
        let (block, remain) = GroupProposal::raw_decode(obj.object_raw.as_slice())?;
        assert_eq!(remain.len(), 0);
        Ok(block)
    }

    pub async fn get_group(
        &self,
        group_id: &ObjectId,
        group_chunk_id: Option<&ObjectId>,
    ) -> BuckyResult<Group> {
        unimplemented!()
    }
}
