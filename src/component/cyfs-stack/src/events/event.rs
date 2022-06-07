use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;
use cyfs_util::*;

use std::fmt;
use std::sync::Arc;

pub struct RouterEvent<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    pub index: i32,

    pub id: String,

    pub routine:
        Box<dyn EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>>,
}

impl<REQ, RESP> RouterEvent<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    pub fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.id == other.id
    }

    pub fn new(
        id: impl Into<String>,
        index: i32,
        routine: Box<
            dyn EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>,
        >,
    ) -> BuckyResult<Self> {
        let event = RouterEvent::<REQ, RESP> {
            id: id.into(),
            index,
            routine,
        };

        Ok(event)
    }
}

pub(crate) struct RouterEventsImpl<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    event_list: Vec<Arc<RouterEvent<REQ, RESP>>>,
}

impl<REQ, RESP> RouterEventsImpl<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    pub fn new() -> Self {
        Self {
            event_list: Vec::new(),
        }
    }

    pub fn listener_count(&self) -> usize {
        self.event_list.len()
    }

    pub fn category() -> RouterEventCategory {
        RouterEventRequest::<REQ>::category()
    }

    pub fn add_event(&mut self, event: RouterEvent<REQ, RESP>) -> BuckyResult<bool> {
        let event = Arc::new(event);

        let changed = (|| {
            for i in 0..self.event_list.len() {
                let cur = &self.event_list[i];
                if cur.id == event.id {
                    // 比较是否相同
                    let changed;
                    if cur.eq(&event) {
                        debug!(
                            "router event already exists! category={}, id={}",
                            Self::category(),
                            event.id
                        );
                        changed = false;
                    } else {
                        info!(
                            "will replace router event: category={}, id={}, index={}",
                            Self::category(),
                            event.id,
                            event.index,
                        );
                        changed = true;
                    }
                    // 无论是否相同，都直接替换，因为routine可能变化了
                    self.event_list[i] = event;
                    return changed;
                }
            }

            info!(
                "new router event: category={}, id={}, index={}",
                Self::category(),
                event.id,
                event.index,
            );
            self.event_list.push(event);

            true
        })();

        if changed {
            // 按照index排序，必须是稳定算法
            self.event_list
                .sort_by(|a, b| a.index.partial_cmp(&b.index).unwrap());
        }

        Ok(changed)
    }

    pub fn remove_event(&mut self, id: &str) -> bool {
        for i in 0..self.event_list.len() {
            if self.event_list[i].id == id {
                info!(
                    "will remove router event: category={}, id={}",
                    Self::category(),
                    id
                );
                self.event_list.remove(i);
                return true;
            }
        }

        warn!(
            "router event not found! category={}, id={}",
            Self::category(),
            id
        );

        false
    }

    // events必须确保已经经过filter了
    pub async fn emit(
        category: &RouterEventCategory,
        event: Arc<RouterEvent<REQ, RESP>>,
        param: &RouterEventRequest<REQ>,
    ) -> RouterEventResponse<RESP> {
        info!(
            "will emit event routine: category={}, id={}, param={}",
            category, event.id, param
        );

        match event.routine.call(&param).await {
            Ok(resp) => {
                info!(
                    "emit event routine success: category={}, id={}, handled={}, call_next={}",
                    category, event.id, resp.handled, resp.call_next,
                );
                resp
            }
            Err(e) => {
                error!(
                    "emit event routine error: category={}, id={}, {}",
                    category, event.id, e
                );

                // 触发事件出错后，使用默认resp
                RouterEventResponse {
                    handled: false,
                    call_next: true,
                    response: None,
                }
            }
        }
    }
}

pub struct RouterEvents<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    events: Arc<Mutex<RouterEventsImpl<REQ, RESP>>>,
}

impl<REQ, RESP> RouterEvents<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(RouterEventsImpl::<REQ, RESP>::new())),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.listener_count() == 0
    }

    pub fn listener_count(&self) -> usize {
        let inner = self.events.lock().unwrap();
        inner.listener_count()
    }

    pub fn add_event(&self, event: RouterEvent<REQ, RESP>) -> BuckyResult<()> {
        let mut inner = self.events.lock().unwrap();
        inner.add_event(event)?;
        Ok(())
    }

    pub fn remove_event(&self, id: &str) -> bool {
        let mut inner = self.events.lock().unwrap();
        inner.remove_event(id)
    }

    pub fn emitter(&self) -> RouterEventEmitter<REQ, RESP> {
        RouterEventEmitter::<REQ, RESP>::new(self)
    }

    pub fn specified_emitter(&self, id: &str) -> Option<RouterEventEmitter<REQ, RESP>> {
        RouterEventEmitter::<REQ, RESP>::new_with_specified(self, id)
    }
}

pub struct RouterEventEmitter<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    category: RouterEventCategory,
    event_list: Vec<Arc<RouterEvent<REQ, RESP>>>,
    next_index: usize,
}

impl<REQ, RESP> RouterEventEmitter<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    pub fn category(&self) -> &RouterEventCategory {
        &self.category
    }

    fn new(events: &RouterEvents<REQ, RESP>) -> Self {
        let events = events.events.lock().unwrap();
        Self {
            category: RouterEventsImpl::<REQ, RESP>::category(),
            event_list: events.event_list.clone(),
            next_index: 0,
        }
    }

    fn new_with_specified(events: &RouterEvents<REQ, RESP>, id: &str) -> Option<Self> {
        let events = events.events.lock().unwrap();
        for event in &events.event_list {
            if event.id == id {
                return Some(Self {
                    category: RouterEventsImpl::<REQ, RESP>::category(),
                    event_list: vec![event.clone()],
                    next_index: 0,
                });
            }
        }

        None
    }

    fn next_event(&mut self) -> Option<Arc<RouterEvent<REQ, RESP>>> {
        while self.next_index < self.event_list.len() {
            let event = &self.event_list[self.next_index];
            self.next_index += 1;

            trace!(
                "will emit event: category={}, id={}, index={}",
                self.category,
                event.id,
                event.index
            );

            return Some(event.clone());
        }

        None
    }

    pub async fn next(&mut self, param: &RouterEventRequest<REQ>) -> RouterEventResponse<RESP> {
        match self.next_event() {
            Some(event) => RouterEventsImpl::emit(&self.category, event, &param).await,
            None => RouterEventResponse {
                handled: false,
                call_next: false,
                response: None,
            },
        }
    }

    pub async fn emit(&mut self, param: REQ) -> RouterEventResponse<RESP> {
        let req = RouterEventRequest {
            request: param,
        };

        let mut last_resp = Some(RouterEventResponse {
            handled: false,
            call_next: false,
            response: None,
        });

        loop {
            let resp = match self.next_event() {
                Some(event) => {
                    RouterEventsImpl::emit(&self.category, event, &req).await
                }
                None => break last_resp.unwrap(),
            };

            if !resp.call_next {
                break resp;
            } else {
                last_resp = Some(resp)
            }
        }
    }
}
