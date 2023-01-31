use std::sync::Arc;

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId, ObjectTypeCode};
use cyfs_lib::{
    DeviceZoneCategory, DeviceZoneInfo, NONAPILevel, NONGetObjectInputRequest,
    NONInputRequestCommon, NONObjectInfo, NONPostObjectInputRequest, RequestProtocol,
    RequestSourceInfo,
};

use crate::{non::NONInputProcessor, non_api::NONService};

pub struct GroupNONDriver {
    non_service: Arc<NONService>,
}

impl GroupNONDriver {
    pub fn new(non_service: Arc<NONService>) -> Self {
        Self { non_service }
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

                    level: NONAPILevel::Router,

                    target: from.map(|remote| remote.clone()),
                    flags: 0,
                },
                object_id: object_id.clone(),
                inner_path: None,
            })
            .await
            .map(|resp| resp.object)
    }

    async fn post_object(
        &self,
        dec_id: &ObjectId,
        obj: NONObjectInfo,
        to: &ObjectId,
    ) -> BuckyResult<()> {
        self.non_service
            .post_object(NONPostObjectInputRequest {
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

                    target: Some(to.clone()),
                    flags: 0,
                },
                object: obj,
            })
            .await
            .map(|_| ())
    }
}
