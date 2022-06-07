use super::request::*;
use cyfs_base::*;
use cyfs_util::*;

use std::fmt;

#[async_trait::async_trait]
pub(crate) trait RouterEventAnyRoutine: Send + Sync {
    async fn emit(&self, param: String) -> BuckyResult<String>;
}

pub(crate) struct RouterEventRoutineT<REQ, RESP>(
    pub Box<dyn EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>>,
)
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display;

#[async_trait::async_trait]
impl<REQ, RESP> RouterEventAnyRoutine for RouterEventRoutineT<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    async fn emit(&self, param: String) -> BuckyResult<String> {
        let param = RouterEventRequest::<REQ>::decode_string(&param)?;
        self.0
            .call(&param)
            .await
            .map(|resp| JsonCodec::encode_string(&resp))
    }
}
