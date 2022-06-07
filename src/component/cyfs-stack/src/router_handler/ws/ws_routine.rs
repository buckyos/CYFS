use cyfs_base::*;
use cyfs_util::*;
use cyfs_lib::*;

use async_trait::async_trait;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

pub(crate) struct RouterHandlerWebSocketRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    _phantom_req: PhantomData<REQ>,
    _phantom_resp: PhantomData<RESP>,

    chain: RouterHandlerChain,
    categroy: RouterHandlerCategory,
    id: String,

    session_requestor: Arc<WebSocketRequestManager>,
}

#[async_trait]
impl<REQ, RESP>
    EventListenerAsyncRoutine<RouterHandlerRequest<REQ, RESP>, RouterHandlerResponse<REQ, RESP>>
    for RouterHandlerWebSocketRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    async fn call(
        &self,
        param: &RouterHandlerRequest<REQ, RESP>,
    ) -> BuckyResult<RouterHandlerResponse<REQ, RESP>> {
        if self.session_requestor.is_session_valid() {
            let resp = self.post_with_timeout(param).await?;
            Ok(resp)
        } else {
            warn!(
                "handler routine ws session is disconnected, category={}, id={}, sid={}",
                self.categroy,
                self.id,
                self.session_requestor.sid()
            );
            Err(BuckyError::from(BuckyErrorCode::NotConnected))
        }
    }
}

impl<REQ, RESP> RouterHandlerWebSocketRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    pub fn new(
        chain: &RouterHandlerChain,
        categroy: &RouterHandlerCategory,
        id: &str,
        session_requestor: Arc<WebSocketRequestManager>,
    ) -> BuckyResult<Self> {
        info!(
            "new ws router handler routine: chain={}, categroy={}, id={}, sid={}",
            chain,
            categroy,
            id,
            session_requestor.sid()
        );

        let ret = Self {
            _phantom_req: PhantomData,
            _phantom_resp: PhantomData,
            chain: chain.to_owned(),
            categroy: categroy.to_owned(),
            id: id.to_owned(),
            session_requestor,
        };

        Ok(ret)
    }

    async fn post_with_timeout<T>(&self, param: &RouterHandlerRequest<REQ, RESP>) -> BuckyResult<T>
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

    async fn post<T>(&self, param: &RouterHandlerRequest<REQ, RESP>) -> BuckyResult<T>
    where
        T: JsonCodec<T>,
    {
        let full_param = RouterWSHandlerEventParam::encode_json_impl::<
            RouterHandlerRequest<REQ, RESP>,
        >(&self.chain, &self.categroy, &self.id, param);
        let msg = JsonCodecHelper::encode_string(full_param);

        let resp_str = self
            .session_requestor
            .post_req(ROUTER_WS_HANDLER_CMD_EVENT, msg)
            .await
            .map_err(|e| {
                error!(
                    "emit router handler event error: category={}, id={}, {}",
                    self.categroy, self.id, e
                );
                e
            })?;

        T::decode_string(&resp_str)
    }
}
