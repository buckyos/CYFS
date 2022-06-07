use super::super::manager::AclMatchInstanceRef;
use super::super::request::AclRequest;
use super::desc::*;
use super::device::*;
use super::object::*;
use super::AclSpecifiedRelation;
use cyfs_base::*;


#[derive(Clone)]
pub(crate) struct AclRelationFactory {
    match_instance: AclMatchInstanceRef,
}

impl AclRelationFactory {
    pub fn new(match_instance: AclMatchInstanceRef) -> Self {
        Self { match_instance }
    }

    pub async fn new_relation(
        &self,
        desc: &AclRelationDescription,
        req: &dyn AclRequest,
    ) -> BuckyResult<Box<dyn AclSpecifiedRelation>> {
        let r = match desc.category {
            AclRelationCategory::Device => match desc.what {
                AclRelationWhat::Device => match desc.who {
                    AclRelationWho::My => {
                        let device_id = self.match_instance.zone_manager.get_current_device_id();
                        let ret = AclSpecifiedDeviceRelation::new(device_id.to_owned())?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                    _ => {
                        let ret = AclSpecifiedDeviceRelation::new(req.device().to_owned())?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                },
                AclRelationWhat::Friend => match desc.who {
                    AclRelationWho::My => {
                        let ret =
                            AclFriendDeviceRelation::new_my_friend(self.match_instance.clone())
                                .await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                    _ => {
                        let ret = AclFriendDeviceRelation::new_from_device(
                            self.match_instance.clone(),
                            req.device(),
                        )
                        .await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                },
                AclRelationWhat::Ood => match desc.who {
                    AclRelationWho::My => {
                        let ret =
                            AclOodDeviceRelation::new_my_ood(self.match_instance.clone()).await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                    _ => {
                        let ret = AclOodDeviceRelation::new_from_device(
                            self.match_instance.clone(),
                            req.device(),
                        )
                        .await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                },
                AclRelationWhat::Zone => match desc.who {
                    AclRelationWho::My => {
                        let ret =
                            AclZoneDeviceRelation::new_my_zone(self.match_instance.clone()).await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                    _ => {
                        let ret = AclZoneDeviceRelation::new_from_device(
                            self.match_instance.clone(),
                            req.device(),
                        )
                        .await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                },
            },
            AclRelationCategory::Object => match desc.what {
                AclRelationWhat::Device => match desc.who {
                    AclRelationWho::My => {
                        let device_id = self.match_instance.zone_manager.get_current_device_id();
                        let ret = AclSpecifiedObjectRelation::new(
                            self.match_instance.clone(),
                            device_id.to_owned(),
                        )?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                    _ => {
                        let ret = AclSpecifiedObjectRelation::new(
                            self.match_instance.clone(),
                            req.device().to_owned(),
                        )?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                },
                AclRelationWhat::Friend => match desc.who {
                    AclRelationWho::My => {
                        let ret =
                            AclFriendObjectRelation::new_my_friend(self.match_instance.clone())
                                .await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                    _ => {
                        let ret = AclFriendObjectRelation::new_from_device(
                            self.match_instance.clone(),
                            req.device(),
                        )
                        .await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                },
                AclRelationWhat::Ood => match desc.who {
                    AclRelationWho::My => {
                        let ret =
                            AclOodDeviceRelation::new_my_ood(self.match_instance.clone()).await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                    _ => {
                        let ret = AclOodDeviceRelation::new_from_device(
                            self.match_instance.clone(),
                            req.device(),
                        )
                        .await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                },
                AclRelationWhat::Zone => match desc.who {
                    AclRelationWho::My => {
                        let ret =
                            AclZoneObjectRelation::new_my_zone(self.match_instance.clone()).await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                    _ => {
                        let ret = AclZoneObjectRelation::new_from_device(
                            self.match_instance.clone(),
                            req.device(),
                        )
                        .await?;
                        Box::new(ret) as Box<dyn AclSpecifiedRelation>
                    }
                },
            },
        };

        Ok(r)
    }
}
