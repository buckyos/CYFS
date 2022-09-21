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
        let dec_id: Option<ObjectId> = Self::dec_id_from_request(&request)?;

        NONInputHttpRequest::extract_source_device(zone_manager, protocol, request, dec_id).await
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
