use super::super::{RouterEvent, RouterEventsManager};
use super::ws_routine::RouterEventWebSocketRoutine;
use cyfs_base::*;
use cyfs_util::*;
use cyfs_lib::*;

use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct RouterEventWSProcessor {
    manager: RouterEventsManager,
}

impl RouterEventWSProcessor {
    pub fn new(manager: RouterEventsManager) -> Self {
        Self { manager }
    }

    fn create_event<REQ, RESP>(
        session_requestor: Arc<WebSocketRequestManager>,
        req: &RouterWSAddEventParam,
    ) -> BuckyResult<RouterEvent<REQ, RESP>>
    where
        REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
        RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
        RouterEventRequest<REQ>: RouterEventCategoryInfo,
    {
        info!(
            "new router ws event: sid={}, category={}, id={}, routine={}",
            session_requestor.sid(),
            req.category.to_string(),
            req.id,
            req.routine
        );

        let routine = Box::new(RouterEventWebSocketRoutine::<REQ, RESP>::new(
            &req.category,
            &req.id,
            session_requestor.clone(),
        )?)
            as Box<
                dyn EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>,
            >;

        let event = RouterEvent::new(req.id.clone(), req.index, routine)?;

        Ok(event)
    }

    pub async fn on_add_event_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        req: &RouterWSAddEventParam,
    ) -> BuckyResult<()> {
        match req.category {
            RouterEventCategory::TestEvent => {
                let event = Self::create_event::<TestEventRequest, TestEventResponse>(
                    session_requestor,
                    &req,
                )?;
                self.manager.events().test_event().add_event(event)
            }
        }
    }

    pub fn on_remove_event_request(&self, req: RouterWSRemoveEventParam) -> BuckyResult<bool> {
        let ret = match req.category {
            RouterEventCategory::TestEvent => {
                self.manager.events().test_event().remove_event(&req.id)
            }
        };

        Ok(ret)
    }
}
