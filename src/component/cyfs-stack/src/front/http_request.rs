use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

pub(crate) struct FrontInputHttpRequest<State> {
    pub request: tide::Request<State>,

    pub source: RequestSourceInfo,
}

impl<State> FrontInputHttpRequest<State> {
    pub async fn new(
        zone_manager: &ZoneManagerRef,
        protocol: &NONProtocol,
        request: tide::Request<State>,
    ) -> Result<Self, tide::Response> {
        let source: DeviceId =
            RequestorHelper::decode_header(&request, ::cyfs_base::CYFS_REMOTE_DEVICE).unwrap();
        let dec_id: Option<ObjectId> =
            Self::dec_id_from_request(&request).map_err(|e| RequestorHelper::trans_error::<tide::Response>(e))?;

        let mut source = zone_manager
            .resolve_source_info(&dec_id, source)
            .await
            .map_err(|e| RequestorHelper::trans_error::<tide::Response>(e))?;

        source.protocol = *protocol;

        Ok(Self {
            source, request,
        })
    }

    fn dec_id_from_request(req: &tide::Request<State>) -> BuckyResult<Option<ObjectId>> {
        // first extract dec_id from headers
        match RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_DEC_ID)? {
            Some(dec_id) => Ok(Some(dec_id)),
            None => {
                // try extract dec_id from query pairs
                let dec_id = match RequestorHelper::value_from_querys("dec_id", req.url()) {
                    Ok(v) => v,
                    Err(e) => {
                        let msg = format!(
                            "invalid request url dec_id query param! {}, {}",
                            req.url(),
                            e
                        );
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                    }
                };

                Ok(dec_id)
            }
        }
    }
}
