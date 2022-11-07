use crate::{non::NONInputHttpRequest, zone::ZoneManagerRef};
use cyfs_base::*;
use cyfs_lib::*;

pub(crate) struct FrontInputHttpRequest<State> {
    pub request: tide::Request<State>,

    pub source: RequestSourceInfo,
}

impl<State> FrontInputHttpRequest<State> {
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
        let dec_id: Option<ObjectId> = RequestorHelper::dec_id_from_request(&request)?;

        NONInputHttpRequest::extract_source_device(zone_manager, protocol, request, dec_id).await
    }
}
