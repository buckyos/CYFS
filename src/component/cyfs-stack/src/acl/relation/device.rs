use super::super::{manager::AclMatchInstanceRef, request::*};
use super::helper::*;
use super::AclSpecifiedRelation;
use cyfs_base::*;
use cyfs_core::*;


/*
"xxx friend device",
"xxx zone device",
"xxx ood device",
"xxx device",
其中xxx = my/source/target
*/

// xxx-zone's device
pub(crate) struct AclZoneDeviceRelation {
    zone_id: ZoneId,
    match_instance: AclMatchInstanceRef,
}

impl AclZoneDeviceRelation {
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
impl AclSpecifiedRelation for AclZoneDeviceRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        let zone = self
            .match_instance
            .zone_manager
            .query(&self.zone_id)
            .unwrap();
        let ret = zone.is_known_device(req.device());

        Ok(ret)
    }
}

// xxx-ood's device
pub(crate) struct AclOodDeviceRelation {
    zone_id: ZoneId,

    // 如果当前device就是ood，那么这里缓存一份
    current_ood_id: Option<DeviceId>,

    match_instance: AclMatchInstanceRef,
}

impl AclOodDeviceRelation {
    pub async fn new_from_device(
        match_instance: AclMatchInstanceRef,
        device_id: &DeviceId,
    ) -> BuckyResult<Self> {
        let zone = match_instance
            .zone_manager
            .get_zone(device_id, None)
            .await?;
        let current_ood_id = if zone.is_ood(device_id) {
            Some(device_id.clone())
        } else {
            None
        };

        let ret = Self {
            zone_id: zone.zone_id(),
            current_ood_id,
            match_instance,
        };

        Ok(ret)
    }

    pub async fn new_my_ood(match_instance: AclMatchInstanceRef) -> BuckyResult<Self> {
        let info = match_instance.zone_manager.get_current_info().await?;
        let current_ood_id = if info.zone_role.is_ood_device() {
            Some(info.device_id.clone())
        } else {
            None
        };

        let ret = Self {
            zone_id: info.zone_id.clone(),
            current_ood_id,
            match_instance,
        };

        Ok(ret)
    }
}

#[async_trait::async_trait]
impl AclSpecifiedRelation for AclOodDeviceRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        let device = req.device();
        if let Some(ood) = &self.current_ood_id {
            if device == ood {
                return Ok(true);
            }
        }

        // 多ood情况下，需要通过zone来判断
        // FIXME ood变动应该很少，考虑增加缓存来优化
        let zone = self
            .match_instance
            .zone_manager
            .query(&self.zone_id)
            .unwrap();
        let ret = zone.is_ood(req.device());

        Ok(ret)
    }
}

// my-friend's device
pub(crate) struct AclFriendDeviceRelation {
    relation: AclFriendRelation,
}

impl AclFriendDeviceRelation {
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
impl AclSpecifiedRelation for AclFriendDeviceRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        self.relation.is_friend_device(req.device()).await
    }
}

pub(crate) struct AclSpecifiedDeviceRelation {
    device_id: DeviceId,
}

impl AclSpecifiedDeviceRelation {
    pub fn new(device_id: DeviceId) -> BuckyResult<Self> {
        let ret = Self { device_id };

        Ok(ret)
    }
}

#[async_trait::async_trait]
impl AclSpecifiedRelation for AclSpecifiedDeviceRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        Ok(self.device_id == *req.device())
    }
}
