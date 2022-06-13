use super::category::*;
use super::request::*;
use super::ws::*;
use crate::stack::*;
use cyfs_base::*;
use cyfs_util::*;

use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

#[async_trait]
pub(crate) trait RouterEventAnyRoutine: Send + Sync {
    async fn emit(&self, param: String) -> BuckyResult<String>;
}

pub(crate) struct RouterEventRoutineT<REQ, RESP>(
    pub Box<dyn EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>>,
)
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display;

#[async_trait]
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

#[derive(Clone)]
pub struct RouterEventManager {
    dec_id: Option<SharedObjectStackDecID>,

    inner: RouterWSEventManager,
}

impl RouterEventManager {
    pub async fn new(dec_id: Option<SharedObjectStackDecID>, _service_url: &str, event_type: CyfsStackEventType) -> BuckyResult<Self> {
        let inner = match event_type {
            CyfsStackEventType::Http => {
                unimplemented!();
            }
            CyfsStackEventType::WebSocket(ws_url) => {
                let ret = RouterWSEventManager::new(ws_url);
                ret.start();

                ret
            }
        };

        Ok(Self { dec_id, inner })
    }

    pub fn clone_processor(&self) -> RouterEventManagerProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    fn get_dec_id(&self) -> Option<ObjectId> {
        self.dec_id.as_ref().map(|v| v.get().cloned()).flatten()
    }

    pub fn add_event<REQ, RESP>(
        &self,
        id: &str,
        index: i32,
        routine: Box<
            dyn EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>,
        >,
    ) -> BuckyResult<()>
    where
        REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
        RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
        RouterEventRequest<REQ>: RouterEventCategoryInfo,
    {
        self.inner.add_event(id, self.get_dec_id(), index, routine)
    }

    pub async fn remove_event(&self, category: RouterEventCategory, id: &str) -> BuckyResult<bool> {
        self.inner.remove_event(category, id, self.get_dec_id()).await
    }
}

use super::processor::*;

#[async_trait::async_trait]
impl<REQ, RESP> RouterEventProcessor<REQ, RESP> for RouterEventManager
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    async fn add_event(
        &self,
        id: &str,
        index: i32,
        routine: Box<
            dyn EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>,
        >,
    ) -> BuckyResult<()> {
        Self::add_event(&self, id, index, routine)
    }

    async fn remove_event(&self, id: &str) -> BuckyResult<bool> {
        let category = extract_router_event_category::<RouterEventRequest<REQ>>();
        Self::remove_event(&self, category, id).await
    }
}

impl RouterEventManagerProcessor for RouterEventManager {
    fn test_event(&self) -> &dyn RouterEventProcessor<TestEventRequest, TestEventResponse> {
        self
    }
}
