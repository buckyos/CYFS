use std::{collections::HashMap, sync::Arc, time::Instant};

use async_std::sync::RwLock;
use cyfs_base::{
    AnyNamedObject, BuckyError, BuckyErrorCode, BuckyResult, Device, DeviceId, NamedObject,
    ObjectDesc, ObjectId, ObjectTypeCode, People, PeopleId, RawConvertTo, RawDecode, RawFrom,
    TypelessCoreObject,
};

use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal, GroupQuorumCertificate,
};
use cyfs_lib::NONObjectInfo;

use crate::{MEMORY_CACHE_DURATION, MEMORY_CACHE_SIZE};

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
        to: Option<&ObjectId>,
    ) -> BuckyResult<Option<NONObjectInfo>>;
}

#[derive(Clone)]
pub(crate) struct NONDriverHelper {
    driver: Arc<Box<dyn NONDriver>>,
    dec_id: ObjectId,
    cache: NONObjectCache,
    local_device_id: ObjectId,
}

impl NONDriverHelper {
    pub fn new(
        driver: Arc<Box<dyn NONDriver>>,
        dec_id: ObjectId,
        local_device_id: ObjectId,
    ) -> Self {
        Self {
            driver,
            dec_id,
            cache: NONObjectCache::new(),
            local_device_id,
        }
    }

    pub fn dec_id(&self) -> &ObjectId {
        &self.dec_id
    }

    pub async fn get_object(
        &self,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<NONObjectInfo> {
        if let Some(obj) = self.cache.find_in_cache(object_id).await {
            return Ok(obj);
        }

        {
            let result = self
                .driver
                .get_object(&self.dec_id, object_id, from)
                .await?;

            self.cache.insert_cache(&result).await;

            Ok(result)
        }
    }

    pub async fn put_object(&self, obj: NONObjectInfo) -> BuckyResult<()> {
        self.driver.put_object(&self.dec_id, obj).await
    }

    pub async fn post_object(
        &self,
        obj: NONObjectInfo,
        to: Option<&ObjectId>,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        self.driver.post_object(&self.dec_id, obj, to).await
    }

    pub async fn broadcast(&self, obj: NONObjectInfo, to: &[ObjectId]) {
        futures::future::join_all(to.iter().map(|to| self.post_object(obj.clone(), Some(to))))
            .await;
    }

    pub async fn get_block(
        &self,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<GroupConsensusBlock> {
        // TODO: remove block without signatures
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

    pub async fn get_qc(
        &self,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<GroupQuorumCertificate> {
        let obj = self.get_object(object_id, from).await?;
        let (block, remain) = GroupQuorumCertificate::raw_decode(obj.object_raw.as_slice())?;
        assert_eq!(remain.len(), 0);
        Ok(block)
    }

    pub async fn put_qc(&self, qc: &GroupQuorumCertificate) -> BuckyResult<()> {
        let buf = qc.to_vec()?;
        let block_any = Arc::new(AnyNamedObject::Core(
            TypelessCoreObject::clone_from_slice(buf.as_slice()).unwrap(),
        ));

        let qc = NONObjectInfo {
            object_id: qc.desc().object_id(),
            object_raw: qc.to_vec()?,
            object: Some(block_any),
        };
        self.put_object(qc).await?;
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

    pub async fn load_all_proposals_for_block(
        &self,
        block: &GroupConsensusBlock,
        proposals_map: &mut HashMap<ObjectId, GroupProposal>,
    ) -> BuckyResult<()> {
        let non_driver = self.clone();
        let block_owner = block.owner().clone();

        let remote = match block_owner.obj_type_code() {
            cyfs_base::ObjectTypeCode::Device => DeviceId::try_from(block_owner).unwrap(),
            cyfs_base::ObjectTypeCode::People => {
                let people_id = PeopleId::try_from(block_owner).unwrap();
                match self.get_ood(&people_id).await {
                    Ok(device_id) => device_id,
                    Err(e) => {
                        log::warn!(
                            "[non driver] load_all_proposals_for_block get ood of {}, failed err: {:?}",
                            block_owner,
                            e
                        );
                        return Err(e);
                    }
                }
            }
            _ => panic!("invalid remote type: {:?}", block_owner.obj_type_code()),
        };

        log::debug!(
            "{} load_all_proposals_for_block {} from {}",
            self.local_device_id,
            block.block_id(),
            remote
        );

        let load_futs = block.proposals().iter().map(|proposal| {
            let proposal_id = proposal.proposal;
            let non_driver = non_driver.clone();
            let remote = remote.clone();
            async move {
                let ret = non_driver
                    .get_proposal(&proposal_id, Some(remote.object_id()))
                    .await;

                log::debug!(
                    "{} load_all_proposals_for_block {}/{} from {}, ret: {:?}",
                    self.local_device_id,
                    block.block_id(),
                    proposal_id,
                    remote,
                    ret.as_ref().map(|_| ())
                );

                ret
            }
        });

        let mut results = futures::future::join_all(load_futs).await;
        let proposal_count = results.len();

        for i in 0..proposal_count {
            let result = results.pop().unwrap();
            let proposal_id = block
                .proposals()
                .get(proposal_count - i - 1)
                .unwrap()
                .proposal;
            match result {
                Ok(proposal) => {
                    assert_eq!(proposal_id, proposal.desc().object_id());
                    proposals_map.insert(proposal_id, proposal);
                }
                Err(err) => return Err(err),
            }
        }

        Ok(())
    }
}

#[derive(Clone)]
struct NONObjectCache {
    cache: Arc<RwLock<(HashMap<ObjectId, NONObjectInfo>, Instant)>>,
    cache_1: Arc<RwLock<HashMap<ObjectId, NONObjectInfo>>>,
}

impl NONObjectCache {
    fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new((HashMap::new(), Instant::now()))),
            cache_1: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn find_in_cache(&self, object_id: &ObjectId) -> Option<NONObjectInfo> {
        {
            let cache = self.cache.read().await;
            if let Some(obj) = cache.0.get(object_id) {
                return Some(obj.clone());
            }
        }

        {
            let cache = self.cache_1.read().await;
            cache.get(object_id).cloned()
        }
    }

    async fn insert_cache(&self, obj: &NONObjectInfo) {
        let new_cache_1 = {
            let mut cache = self.cache.write().await;
            let now = Instant::now();
            if now.duration_since(cache.1) > MEMORY_CACHE_DURATION
                || cache.0.len() > MEMORY_CACHE_SIZE
            {
                let mut new_cache = HashMap::new();
                std::mem::swap(&mut new_cache, &mut cache.0);
                cache.1 = now;
                cache.0.insert(obj.object_id, obj.clone());
                new_cache
            } else {
                cache.0.insert(obj.object_id, obj.clone());
                return;
            }
        };

        {
            let mut cache_1 = self.cache_1.write().await;
            *cache_1 = new_cache_1;
        }
    }
}
