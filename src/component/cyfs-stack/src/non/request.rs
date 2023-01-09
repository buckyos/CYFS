use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

pub(crate) struct NONInputHttpRequest<State> {
    pub request: tide::Request<State>,

    pub source: RequestSourceInfo,
}

impl<State> NONInputHttpRequest<State> {
    pub async fn new(
        zone_manager: &ZoneManagerRef,
        protocol: &RequestProtocol,
        request: tide::Request<State>,
    ) -> Result<Self, tide::Response> {
        let source = Self::extract_source(zone_manager, protocol, &request)
            .await
            .map_err(|e| RequestorHelper::trans_error::<tide::Response>(e))?;

        Ok(Self { request, source })
    }

    async fn extract_source(
        zone_manager: &ZoneManagerRef,
        protocol: &RequestProtocol,
        request: &tide::Request<State>,
    ) -> BuckyResult<RequestSourceInfo> {
        let dec_id: Option<ObjectId> =
        RequestorHelper::decode_optional_header(request, cyfs_base::CYFS_DEC_ID)?;

        Self::extract_source_device(zone_manager, protocol, request, dec_id).await
    }

    pub async fn extract_source_device(
        zone_manager: &ZoneManagerRef,
        protocol: &RequestProtocol,
        request: &tide::Request<State>,
        dec_id: Option<ObjectId>,
    ) -> BuckyResult<RequestSourceInfo> {
        let source: DeviceId =
            RequestorHelper::decode_header(request, ::cyfs_base::CYFS_REMOTE_DEVICE).unwrap();
        
        let mut info = zone_manager.resolve_source_info(&dec_id, source).await?;

        let origin_source: Option<DeviceId> =
            RequestorHelper::decode_optional_header(request, cyfs_base::CYFS_SOURCE)?;
        if let Some(origin_source) = origin_source {
            if info.is_current_zone() && origin_source != *info.zone.device.as_ref().unwrap() {
                info = zone_manager
                    .resolve_source_info(&dec_id, origin_source)
                    .await?;
            }
        }

        info.protocol = *protocol;

        Ok(info)
    }
}
