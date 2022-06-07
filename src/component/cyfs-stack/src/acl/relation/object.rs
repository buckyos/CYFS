use super::super::{manager::AclMatchInstanceRef, request::*};
use super::helper::*;
use super::AclSpecifiedRelation;
use cyfs_base::*;
use cyfs_core::*;

use std::convert::TryFrom;

pub(crate) struct AclZoneObjectRelation {
    zone_id: ZoneId,
    match_instance: AclMatchInstanceRef,
}

impl AclZoneObjectRelation {
    pub async fn new_from_device(
        match_instance: AclMatchInstanceRef,
        device_id: &DeviceId,
    ) -> BuckyResult<Self> {
        let zone_id = match_instance
            .zone_manager
            .get_zone_id(device_id, None)
            .await?;
        let ret = Self {
            zone_id,
            match_instance,
        };
        Ok(ret)
    }

    pub async fn new_my_zone(match_instance: AclMatchInstanceRef) -> BuckyResult<Self> {
        let zone_id = match_instance.zone_manager.get_current_zone_id().await?;
        let ret = Self {
            zone_id,
            match_instance,
        };
        Ok(ret)
    }
}

#[async_trait::async_trait]
impl AclSpecifiedRelation for AclZoneObjectRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        let zone = AclRelationHelper::get_object_zone(&self.match_instance, req).await?;
        match zone {
            Some(zone) => {
                if self.zone_id == zone.zone_id() {
                    info!(
                        "acl req object belong to zone: obj={}, zone={}",
                        req.object_id().unwrap(),
                        self.zone_id
                    );

                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false),
        }
    }
}

// 判断object's owner是不是属于friend_list的device/owner
pub(crate) struct AclFriendObjectRelation {
    relation: AclFriendRelation,
}

impl AclFriendObjectRelation {
    pub async fn new_from_device(
        match_instance: AclMatchInstanceRef,
        device_id: &DeviceId,
    ) -> BuckyResult<Self> {
        Ok(Self {
            relation: AclFriendRelation::new_from_device(match_instance, device_id).await?,
        })
    }

    pub async fn new_my_friend(match_instance: AclMatchInstanceRef) -> BuckyResult<Self> {
        Ok(Self {
            relation: AclFriendRelation::new_my_friend(match_instance).await?,
        })
    }
}

#[async_trait::async_trait]
impl AclSpecifiedRelation for AclFriendObjectRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        match AclRelationHelper::get_object_owner(&self.relation.match_instance, req).await? {
            Some(owner) => {
                let friend: FriendList = self
                    .relation
                    .match_instance
                    .load_object_ex(self.relation.friend_list_id.object_id())
                    .await?;

                if friend.friend_list().contains_key(&owner) {
                    info!("acl req object's owner is in friend list: object={}, owner={}, friend_list={}",
                        req.object_id().unwrap(), owner, self.relation.friend_list_id,
                    );
                    Ok(true)
                } else {
                    debug!("acl req object's owner is not in friend list: object={}, owner={}, friend_list={}",
                        req.object_id().unwrap(), owner, self.relation.friend_list_id,
                    );
                    Ok(false)
                }
            }
            None => Ok(false),
        }
    }
}

// object's owner必须是指定的ood(ood_list)
pub(crate) struct AclOodObjectRelation {
    zone_id: ZoneId,
    match_instance: AclMatchInstanceRef,
}

impl AclOodObjectRelation {
    pub async fn new_from_device(
        match_instance: AclMatchInstanceRef,
        device_id: &DeviceId,
    ) -> BuckyResult<Self> {
        let zone = match_instance
            .zone_manager
            .get_zone(device_id, None)
            .await?;

        let ret = Self {
            zone_id: zone.zone_id(),
            match_instance,
        };

        Ok(ret)
    }

    pub async fn new_my_ood(match_instance: AclMatchInstanceRef) -> BuckyResult<Self> {
        let info = match_instance.zone_manager.get_current_info().await?;

        let ret = Self {
            zone_id: info.zone_id.clone(),
            match_instance,
        };

        Ok(ret)
    }
}

#[async_trait::async_trait]
impl AclSpecifiedRelation for AclOodObjectRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        match AclRelationHelper::get_object_owner(&self.match_instance, req).await? {
            Some(owner) => {
                // owner必须是device类型(ood-device)
                if owner.obj_type_code() != ObjectTypeCode::Device {
                    return Ok(false);
                }
                let owner = DeviceId::try_from(&owner).unwrap();

                let zone = self
                    .match_instance
                    .zone_manager
                    .query(&self.zone_id)
                    .unwrap();

                let ret = zone.is_ood(&owner);
                if ret {
                    info!(
                        "acl req object's owner is specified ood: object={}, owner={}, zone={}",
                        req.object_id().unwrap(),
                        owner,
                        self.zone_id,
                    );
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false),
        }
    }
}

// object's owner必须是特定device
pub(crate) struct AclSpecifiedObjectRelation {
    device_id: DeviceId,
    match_instance: AclMatchInstanceRef,
}

impl AclSpecifiedObjectRelation {
    pub fn new(match_instance: AclMatchInstanceRef, device_id: DeviceId) -> BuckyResult<Self> {
        let ret = Self {
            device_id,
            match_instance,
        };

        Ok(ret)
    }
}

#[async_trait::async_trait]
impl AclSpecifiedRelation for AclSpecifiedObjectRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        match AclRelationHelper::get_object_owner(&self.match_instance, req).await? {
            Some(owner) => {
                if owner == self.device_id {
                    info!(
                        "acl req object's owner is specified device: object={}, device={}",
                        req.object_id().unwrap(),
                        owner,
                    );
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false),
        }
    }
}
