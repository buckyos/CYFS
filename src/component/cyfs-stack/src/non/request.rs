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
        let source: DeviceId =
            RequestorHelper::decode_header(&request, ::cyfs_base::CYFS_REMOTE_DEVICE).unwrap();
        let dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(&request, cyfs_base::CYFS_DEC_ID)
                .map_err(|e| RequestorHelper::trans_error::<tide::Response>(e))?;

        let mut source = zone_manager
            .resolve_source_info(&dec_id, source)
            .await
            .map_err(|e| RequestorHelper::trans_error::<tide::Response>(e))?;

        source.protocol = *protocol;

        Ok(Self { request, source })
    }
}
