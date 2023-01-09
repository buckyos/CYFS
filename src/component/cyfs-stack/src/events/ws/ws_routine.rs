use cyfs_base::*;
use cyfs_util::*;
use cyfs_lib::*;

use async_trait::async_trait;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

pub(crate) struct RouterEventWebSocketRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    _phantom_req: PhantomData<REQ>,
    _phantom_resp: PhantomData<RESP>,

    categroy: RouterEventCategory,
    id: String,

    session_requestor: Arc<WebSocketRequestManager>,
}

#[async_trait]
impl<REQ, RESP> EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>
    for RouterEventWebSocketRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    async fn call(
        &self,
        param: &RouterEventRequest<REQ>,
    ) -> BuckyResult<RouterEventResponse<RESP>> {
        if self.session_requestor.is_session_valid() {
            let resp = self.post_with_timeout(param).await?;
            Ok(resp)
        } else {
            warn!(
                "router event routine ws session is disconnected, category={}, id={}, sid={}",
                self.categroy,
                self.id,
                self.session_requestor.sid()
            );
            Err(BuckyError::from(BuckyErrorCode::NotConnected))
        }
    }
}

impl<REQ, RESP> RouterEventWebSocketRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    pub fn new(
        categroy: &RouterEventCategory,
        id: &str,
        session_requestor: Arc<WebSocketRequestManager>,
    ) -> BuckyResult<Self> {
        debug!(
            "new ws router event routine: categroy={}, id={}, sid={}",
            categroy,
            id,
            session_requestor.sid()
        );

        let ret = Self {
            _phantom_req: PhantomData,
            _phantom_resp: PhantomData,
            categroy: categroy.to_owned(),
            id: id.to_owned(),
            session_requestor,
        };

        Ok(ret)
    }

    async fn post_with_timeout<T>(&self, param: &RouterEventRequest<REQ>) -> BuckyResult<T>
    where
        T: JsonCodec<T>,
    {
        match async_std::future::timeout(ROUTER_HANDLER_ROUTINE_TIMEOUT.clone(), async {
            self.post(param).await
        })
        .await
        {
            Ok(ret) => ret,
            Err(async_std::future::TimeoutError { .. }) => {
                let msg = format!("emit http routine timeout! id={}", self.id);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Timeout, msg))
            }
        }
    }

    async fn post<T>(&self, param: &RouterEventRequest<REQ>) -> BuckyResult<T>
    where
        T: JsonCodec<T>,
    {
        let full_param = RouterWSEventEmitParam::encode_json_impl::<RouterEventRequest<REQ>>(
            &self.categroy,
            &self.id,
            param,
        );
        let msg = JsonCodecHelper::encode_string(full_param);

        let resp_str = self
            .session_requestor
            .post_req(ROUTER_WS_EVENT_CMD_EVENT, msg)
            .await
            .map_err(|e| {
                error!(
                    "emit router event error: category={}, id={}, {}",
                    self.categroy, self.id, e
                );
                e
            })?;

        T::decode_string(&resp_str)
    }
}
