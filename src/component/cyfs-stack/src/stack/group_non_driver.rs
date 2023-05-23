use std::{sync::Arc, time::Duration};

use cyfs_base::{AccessString, BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_lib::{
    DeviceZoneCategory, DeviceZoneInfo, NONAPILevel, NONGetObjectInputRequest,
    NONInputRequestCommon, NONObjectInfo, NONPostObjectInputRequest, NONPutObjectInputRequest,
    RequestGlobalStatePath, RequestProtocol, RequestSourceInfo,
};
use futures::FutureExt;

use crate::{non::NONInputProcessor, non_api::NONService};

const TIMEOUT_HALF: Duration = Duration::from_millis(2000);

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

    async fn get_object_impl(
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

        let resp = self
            .non_service
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
            .await?;

        // TODO: only set the permissions
        let _ = self.put_object_impl(dec_id, resp.object.clone()).await;
        Ok(resp.object)
    }

    async fn put_object_impl(&self, dec_id: &ObjectId, obj: NONObjectInfo) -> BuckyResult<()> {
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

    async fn post_object_impl(
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

#[async_trait::async_trait]
impl cyfs_group::NONDriver for GroupNONDriver {
    async fn get_object(
        &self,
        dec_id: &ObjectId,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<NONObjectInfo> {
        let fut1 = match futures::future::select(
            self.get_object_impl(dec_id, object_id, from).boxed(),
            async_std::future::timeout(TIMEOUT_HALF, futures::future::pending::<()>()).boxed(),
        )
        .await
        {
            futures::future::Either::Left((ret, _)) => return ret,
            futures::future::Either::Right((_, fut)) => fut,
        };

        log::warn!(
            "group get object timeout (type={:?}) from {:?}, local: {:?}",
            object_id.obj_type_code(),
            from,
            self.local_device_id
        );

        match futures::future::select(
            self.get_object_impl(dec_id, object_id, from).boxed(),
            async_std::future::timeout(TIMEOUT_HALF, fut1).boxed(),
        )
        .await
        {
            futures::future::Either::Left((ret, _)) => ret,
            futures::future::Either::Right((ret, _)) => ret.map_or(
                Err(BuckyError::new(BuckyErrorCode::Timeout, "timeout")),
                |ret| ret,
            ),
        }
    }

    async fn put_object(&self, dec_id: &ObjectId, obj: NONObjectInfo) -> BuckyResult<()> {
        let fut1 = match futures::future::select(
            self.put_object_impl(dec_id, obj.clone()).boxed(),
            async_std::future::timeout(TIMEOUT_HALF, futures::future::pending::<()>()).boxed(),
        )
        .await
        {
            futures::future::Either::Left((ret, _)) => return ret,
            futures::future::Either::Right((_, fut)) => fut,
        };

        log::warn!(
            "group put object timeout (type={:?}/{:?}), local: {:?}",
            obj.object_id.obj_type_code(),
            obj.object.as_ref().map(|o| o.obj_type()),
            self.local_device_id
        );

        match futures::future::select(
            self.put_object_impl(dec_id, obj).boxed(),
            async_std::future::timeout(TIMEOUT_HALF, fut1).boxed(),
        )
        .await
        {
            futures::future::Either::Left((ret, _)) => ret,
            futures::future::Either::Right((ret, _)) => ret.map_or(
                Err(BuckyError::new(BuckyErrorCode::Timeout, "timeout")),
                |ret| ret,
            ),
        }
    }

    async fn post_object(
        &self,
        dec_id: &ObjectId,
        obj: NONObjectInfo,
        to: Option<&ObjectId>,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        let fut1 = match futures::future::select(
            self.post_object_impl(dec_id, obj.clone(), to).boxed(),
            async_std::future::timeout(TIMEOUT_HALF, futures::future::pending::<()>()).boxed(),
        )
        .await
        {
            futures::future::Either::Left((ret, _)) => return ret,
            futures::future::Either::Right((_, fut)) => fut,
        };

        log::warn!(
            "group post object timeout (type={:?}/{:?}) to {:?}, local: {:?}",
            obj.object_id.obj_type_code(),
            obj.object.as_ref().map(|o| o.obj_type()),
            to,
            self.local_device_id
        );

        match futures::future::select(
            self.post_object_impl(dec_id, obj, to).boxed(),
            async_std::future::timeout(TIMEOUT_HALF, fut1).boxed(),
        )
        .await
        {
            futures::future::Either::Left((ret, _)) => ret,
            futures::future::Either::Right((ret, _)) => ret.map_or(
                Err(BuckyError::new(BuckyErrorCode::Timeout, "timeout")),
                |ret| ret,
            ),
        }
    }
}
