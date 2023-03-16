use std::sync::Arc;

use cyfs_base::{AccessString, BuckyResult, ObjectId};
use cyfs_lib::{
    DeviceZoneCategory, DeviceZoneInfo, NONAPILevel, NONGetObjectInputRequest,
    NONInputRequestCommon, NONObjectInfo, NONPostObjectInputRequest, NONPutObjectInputRequest,
    RequestGlobalStatePath, RequestProtocol, RequestSourceInfo,
};

use crate::{non::NONInputProcessor, non_api::NONService};

pub struct GroupNONDriver {
    non_service: Arc<NONService>,
    local_device_id: ObjectId,
}

impl GroupNONDriver {
    pub fn new(non_service: Arc<NONService>, local_device_id: ObjectId) -> Self {
        Self {
            non_service,
            local_device_id,
        }
    }
}

#[async_trait::async_trait]
impl cyfs_group::NONDriver for GroupNONDriver {
    async fn get_object(
        &self,
        dec_id: &ObjectId,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<NONObjectInfo> {
        log::info!(
            "get object {}, local: {}, from: {:?}",
            object_id,
            self.local_device_id,
            from
        );

        self.non_service
            .get_object(NONGetObjectInputRequest {
                common: NONInputRequestCommon {
                    req_path: None,
                    source: RequestSourceInfo {
                        protocol: RequestProtocol::DataBdt,
                        zone: DeviceZoneInfo {
                            device: None,
                            zone: None,
                            zone_category: DeviceZoneCategory::CurrentZone,
                        },
                        dec: dec_id.clone(),
                        verified: None,
                    },

                    level: NONAPILevel::Router, // from.map_or(NONAPILevel::NOC, |_| NONAPILevel::Router),

                    target: from.map(|remote| remote.clone()),
                    flags: 0,
                },
                object_id: object_id.clone(),
                inner_path: None,
            })
            .await
            .map(|resp| resp.object)
    }

    async fn put_object(&self, dec_id: &ObjectId, obj: NONObjectInfo) -> BuckyResult<()> {
        let access = AccessString::full();

        log::info!(
            "put object {} with access {}, local: {}",
            obj.object_id,
            access,
            self.local_device_id
        );

        self.non_service
            .put_object(NONPutObjectInputRequest {
                common: NONInputRequestCommon {
                    req_path: None,
                    source: RequestSourceInfo {
                        protocol: RequestProtocol::DataBdt,
                        zone: DeviceZoneInfo {
                            device: None,
                            zone: None,
                            zone_category: DeviceZoneCategory::CurrentZone,
                        },
                        dec: dec_id.clone(),
                        verified: None,
                    },

                    level: NONAPILevel::Router,

                    target: None,
                    flags: 0,
                },
                object: obj,
                access: Some(AccessString::full()), // TODO access
            })
            .await
            .map(|_| ())
    }

    async fn post_object(
        &self,
        dec_id: &ObjectId,
        obj: NONObjectInfo,
        to: Option<&ObjectId>,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        let obj_type_code = obj.object_id.obj_type_code();
        let obj_type = obj.object.as_ref().map(|obj| obj.obj_type());

        let req_path = RequestGlobalStatePath::new(Some(dec_id.clone()), Some("group/inner-cmd"));

        self.non_service
            .post_object(NONPostObjectInputRequest {
                common: NONInputRequestCommon {
                    req_path: Some(req_path.format_string()),
                    source: RequestSourceInfo {
                        protocol: RequestProtocol::DataBdt,
                        zone: DeviceZoneInfo {
                            device: None,
                            zone: None,
                            zone_category: DeviceZoneCategory::CurrentZone,
                        },
                        dec: dec_id.clone(),
                        verified: None,
                    },

                    level: NONAPILevel::Router, // to.map_or(NONAPILevel::NOC, |_| NONAPILevel::Router),

                    target: to.cloned(),
                    flags: 0,
                },
                object: obj,
            })
            .await
            .map(|resp| resp.object)
            .map_err(|err| {
                log::warn!(
                    "group post object(type={:?}/{:?}) to {:?} failed {:?}",
                    obj_type_code,
                    obj_type,
                    to,
                    err
                );
                err
            })
    }
}
