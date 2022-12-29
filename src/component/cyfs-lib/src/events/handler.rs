use super::category::*;
use super::request::*;
use super::ws::*;
use crate::stack::*;
use cyfs_base::*;
use cyfs_util::*;

use async_trait::async_trait;
use http_types::Url;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
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
    started: Arc<AtomicBool>,
}

impl RouterEventManager {
    pub fn new(dec_id: Option<SharedObjectStackDecID>, ws_url: Url) -> Self {
        let inner = RouterWSEventManager::new(ws_url);

        Self {
            dec_id,
            inner,
            started: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn clone_processor(&self) -> RouterEventManagerProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    fn get_dec_id(&self) -> Option<ObjectId> {
        self.dec_id.as_ref().map(|v| v.get().cloned()).flatten()
    }

    fn try_start(&self) {
        match self
            .started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => {
                info!("will start event manager!");
                self.inner.start()
            }
            Err(_) => {}
        }
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
        info!(
            "will add event: category={}, id={}, index={}",
            extract_router_event_category::<RouterEventRequest<REQ>>(),
            id,
            index
        );

        self.try_start();

        self.inner.add_event(id, self.get_dec_id(), index, routine)
    }

    pub async fn remove_event(&self, category: RouterEventCategory, id: &str) -> BuckyResult<bool> {
        info!("will remove event: category={}, id={}", category, id,);

        self.try_start();

        self.inner
            .remove_event(category, id, self.get_dec_id())
            .await
    }

    pub async fn stop(&self) {
        self.inner.stop().await
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

    fn zone_role_changed_event(
        &self,
    ) -> &dyn RouterEventProcessor<ZoneRoleChangedEventRequest, ZoneRoleChangedEventResponse> {
        self
    }
}
