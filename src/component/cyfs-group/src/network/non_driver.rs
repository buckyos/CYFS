use std::sync::Arc;

use cyfs_base::{
    AnyNamedObject, BuckyError, BuckyErrorCode, BuckyResult, Device, DeviceId, Group, NamedObject,
    ObjectDesc, ObjectId, ObjectTypeCode, People, PeopleId, RawConvertTo, RawDecode, RawFrom,
    TypelessCoreObject,
};
use cyfs_chunk_lib::ChunkMeta;
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal};
use cyfs_lib::NONObjectInfo;

#[async_trait::async_trait]
pub trait NONDriver: Send + Sync {
    async fn get_object(
        &self,
        dec_id: &ObjectId,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<NONObjectInfo>;

    async fn put_object(&self, dec_id: &ObjectId, obj: NONObjectInfo) -> BuckyResult<()>;

    async fn post_object(
        &self,
        dec_id: &ObjectId,
        obj: NONObjectInfo,
        to: &ObjectId,
    ) -> BuckyResult<()>;
}

#[derive(Clone)]
pub(crate) struct NONDriverHelper {
    driver: Arc<Box<dyn NONDriver>>,
    dec_id: ObjectId,
}

impl NONDriverHelper {
    pub fn new(driver: Arc<Box<dyn NONDriver>>, dec_id: ObjectId) -> Self {
        Self { driver, dec_id }
    }

    pub async fn get_object(
        &self,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<NONObjectInfo> {
        self.driver.get_object(&self.dec_id, object_id, from).await
    }

    pub async fn put_object(&self, obj: NONObjectInfo) -> BuckyResult<()> {
        self.driver.put_object(&self.dec_id, obj).await
    }

    pub async fn post_object(&self, obj: NONObjectInfo, to: &ObjectId) -> BuckyResult<()> {
        self.driver.post_object(&self.dec_id, obj, to).await
    }

    pub async fn broadcast(&self, obj: NONObjectInfo, to: &[ObjectId]) {
        futures::future::join_all(to.iter().map(|to| self.post_object(obj.clone(), to))).await;
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

    pub async fn put_block(&self, block: &GroupConsensusBlock) -> BuckyResult<()> {
        let buf = block.to_vec()?;
        let block_any = Arc::new(AnyNamedObject::Core(
            TypelessCoreObject::clone_from_slice(buf.as_slice()).unwrap(),
        ));

        let block = NONObjectInfo {
            object_id: block.block_id().object_id().clone(),
            object_raw: block.to_vec()?,
            object: Some(block_any),
        };
        self.put_object(block).await?;
        Ok(())
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
        from: Option<&ObjectId>,
    ) -> BuckyResult<Group> {
        // TODO: ignore group_chunk_id first
        // match group_chunk_id {
        //     Some(group_chunk_id) => {
        //         let chunk = self.get_object(group_chunk_id, from).await?;
        //         let (group_chunk, remain) = ChunkMeta::raw_decode(chunk.object_raw.as_slice())?;
        //         assert_eq!(remain.len(), 0);
        //         let group = Group::try_from(&group_chunk)?;
        //         if &group.desc().object_id() == group_id {
        //             Ok(group)
        //         } else {
        //             Err(BuckyError::new(BuckyErrorCode::Unmatch, "groupid"))
        //         }
        //     }
        //     None => {
        //         // TODO: latest version from metachain
        //         let group = self.get_object(group_id, from).await?;
        //         let (group, remain) = Group::raw_decode(group.object_raw.as_slice())?;
        //         assert_eq!(remain.len(), 0);
        //         Ok(group)
        //     }
        // }

        let group = self.get_object(group_id, from).await?;
        let (group, remain) = Group::raw_decode(group.object_raw.as_slice())?;
        assert_eq!(remain.len(), 0);
        Ok(group)
    }

    pub async fn get_ood(&self, people_id: &PeopleId) -> BuckyResult<DeviceId> {
        let people = self
            .get_object(people_id.object_id(), Some(people_id.object_id()))
            .await?;
        let (people, remain) = People::raw_decode(people.object_raw.as_slice())?;
        assert_eq!(remain.len(), 0);
        people
            .ood_list()
            .get(0)
            .ok_or(BuckyError::new(BuckyErrorCode::NotFound, "empty ood-list"))
            .map(|d| d.clone())
    }

    pub async fn get_device(&self, device_id: &ObjectId) -> BuckyResult<Device> {
        if let ObjectTypeCode::Device = device_id.obj_type_code() {
            let device = self.get_object(device_id, Some(device_id)).await?;
            let (device, remain) = Device::raw_decode(device.object_raw.as_slice())?;
            assert_eq!(remain.len(), 0);
            Ok(device)
        } else {
            Err(BuckyError::new(BuckyErrorCode::Unmatch, "not device-id"))
        }
    }
}
