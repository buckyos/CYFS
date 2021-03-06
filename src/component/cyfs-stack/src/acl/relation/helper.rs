use super::super::manager::AclMatchInstanceRef;
use super::super::request::*;
use cyfs_base::*;
use cyfs_core::*;


pub(crate) struct AclRelationHelper {}

impl AclRelationHelper {
    pub async fn friend_list_from_device(
        match_instance: &AclMatchInstanceRef,
        device_id: &DeviceId,
    ) -> BuckyResult<(FriendList, FriendListId)> {
        let zone = match_instance
            .zone_manager
            .get_zone(device_id, None)
            .await?;
        let owner = zone.owner();

        let friend_list = FriendList::create(owner.to_owned(), false);
        let friend_list_id: FriendListId = friend_list.desc().calculate_id().try_into().unwrap();

        info!("calc friend_list_id: device={}, owner={}, friend_list={}", device_id, owner, friend_list_id);

        Ok((friend_list, friend_list_id))
    }

    pub async fn my_friend(
        match_instance: &AclMatchInstanceRef,
    ) -> BuckyResult<(FriendList, FriendListId)> {
        let device_id = match_instance
            .zone_manager
            .get_current_device_id()
            .to_owned();
        Self::friend_list_from_device(match_instance, &device_id).await
    }

    pub async fn get_object_zone(
        match_instance: &AclMatchInstanceRef,
        req: &dyn AclRequest,
    ) -> BuckyResult<Option<Zone>> {
        let obj = req.object().await?;
        match obj {
            Some(obj) => {
                match obj.owner() {
                    Some(owner) => {
                        let zone = Self::get_zone(match_instance, owner).await?;
                        Ok(Some(zone))
                    }
                    None => Ok(None),
                }
            }
            None => {
                Ok(None)
            }
        }
    }

    async fn get_zone(match_instance: &AclMatchInstanceRef, owner: &ObjectId) -> BuckyResult<Zone> {
        match owner.obj_type_code() {
            ObjectTypeCode::People | ObjectTypeCode::SimpleGroup => {
                match_instance
                    .zone_manager
                    .get_zone_by_owner(owner, None)
                    .await
            }
            ObjectTypeCode::Device => {
                match_instance
                    .zone_manager
                    .get_zone(&owner.try_into().unwrap(), None)
                    .await
            }
            _ => {
                let msg = format!(
                    "unsupport object's owner type: {:?}, {}",
                    owner.obj_type_code(),
                    owner
                );
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    // ??????object???????????????owner????????????People/SimpleGroup??????????????????device???????????????device
    pub async fn get_object_owner(
        match_instance: &AclMatchInstanceRef,
        req: &dyn AclRequest,
    ) -> BuckyResult<Option<ObjectId>> {
        let obj = req.object().await?;
        if obj.is_none() {
            return Ok(None);
        }

        let obj = obj.as_ref().unwrap();
        match obj.owner() {
            Some(owner) => {
                // ??????zone???????????????owner
                let zone = Self::get_zone(match_instance, owner).await?;
                let owner = zone.owner().to_owned();
                debug!(
                    "acl got object's zone owner: obj={}, zone_owner={}",
                    req.object_id().unwrap(),
                    owner
                );
                Ok(Some(owner))
            }
            None => Ok(None),
        }
    }
}

pub(crate) struct AclFriendRelation {
    pub friend_list_id: FriendListId,
    pub match_instance: AclMatchInstanceRef,
}

impl AclFriendRelation {
    pub async fn new_from_device(
        match_instance: AclMatchInstanceRef,
        device_id: &DeviceId,
    ) -> BuckyResult<Self> {
        let (_, friend_list_id) =
            AclRelationHelper::friend_list_from_device(&match_instance, device_id).await?;

        let ret = Self {
            friend_list_id,
            match_instance,
        };

        Ok(ret)
    }

    pub async fn new_my_friend(match_instance: AclMatchInstanceRef) -> BuckyResult<Self> {
        let (_, friend_list_id) = AclRelationHelper::my_friend(&match_instance).await?;

        let ret = Self {
            friend_list_id,
            match_instance,
        };

        Ok(ret)
    }

    pub async fn is_friend_device(&self, device_id: &DeviceId) -> BuckyResult<bool> {
        // ?????????device???????????????owner(people/simple_group)
        let zone = self
            .match_instance
            .zone_manager
            .get_zone(device_id, None)
            .await?;
        let owner = zone.owner();

        // ?????????owner??????????????????friendlist??????
        // FIXME ?????????friendlist???????????????????????????????????????????????????????????????
        let friend: FriendList = self
            .match_instance
            .load_object_ex(self.friend_list_id.object_id())
            .await?;

        
        let mut ret = false;
        if friend.friend_list().contains_key(owner) {
            info!(
                "acl req device's owner is in friend list: device={}, owner={}, friend_list={}",
                device_id, owner, self.friend_list_id,
            );

            ret = true;
        } else if friend.desc().owner().as_ref() == Some(owner) {
            // ??????device???owner??????friendlist???ower???????????????????????????????????????????????????
            info!(
                "acl req device's owner is friend list's owner: device={}, owner={}, friend_list={}",
                device_id, owner, self.friend_list_id,
            );

            ret = true;
        }


        Ok(ret)
    }
}
