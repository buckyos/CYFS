use super::super::*;
use super::request::*;
use crate::base::*;
use crate::ws::*;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_util::*;

use async_std::task;
use http_types::Url;
use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct RouterEventId {
    category: RouterEventCategory,
    id: String,
}

struct RouterEventItem {
    category: RouterEventCategory,
    id: String,
    dec_id: Option<ObjectId>,
    index: i32,
    routine: Box<dyn RouterEventAnyRoutine>,
}

impl RouterEventItem {
    async fn emit(&self, param: String) -> BuckyResult<String> {
        self.routine.emit(param).await
    }

    async fn register(&self, requestor: &Arc<WebSocketRequestManager>) -> BuckyResult<()> {
        info!(
            "will add ws router event: category={}, id={}, index={}, sid={}",
            self.id,
            self.category,
            self.index,
            requestor.sid(),
        );

        let req = RouterWSAddEventParam {
            category: self.category.clone(),
            id: self.id.clone(),
            dec_id: self.dec_id.clone(),
            index: self.index,
            routine: requestor.sid().to_string(),
        };

        let msg = req.encode_string();
        let resp = requestor
            .post_req(ROUTER_WS_EVENT_CMD_ADD, msg)
            .await
            .map_err(|e| {
                error!(
                    "ws add event failed! category={}, id={}, {}",
                    req.category, req.id, e
                );
                e
            })?;

        let resp = RouterWSEventResponse::decode_string(&resp).map_err(|e| {
            error!(
                "decode add ws router event resp failed! resp={}, {}",
                resp, e
            );
            e
        })?;

        if resp.err == 0 {
            info!(
                "add ws router event success: category={}, id={}, index={}",
                req.category, req.id, self.index,
            );
            Ok(())
        } else {
            error!(
                "add ws router event failed! category={}, id={}, err={}, msg={:?}",
                req.category, req.id, resp.err, resp.msg
            );

            Err(BuckyError::new(resp.err, resp.msg.unwrap_or("".to_owned())))
        }
    }
}

struct RouterEventUnregisterItem {
    category: RouterEventCategory,
    id: String,
    dec_id: Option<ObjectId>,
}

impl RouterEventUnregisterItem {
    async fn unregister(&self, requestor: &Arc<WebSocketRequestManager>) -> BuckyResult<bool> {
        info!(
            "ws will remove event: category={}, id={}, sid={}",
            self.category,
            self.id,
            requestor.sid()
        );

        let req = RouterWSRemoveEventParam {
            category: self.category.clone(),
            id: self.id.clone(),
            dec_id: self.dec_id.clone(),
        };

        let msg = req.encode_string();
        let resp = requestor
            .post_req(ROUTER_WS_EVENT_CMD_REMOVE, msg)
            .await
            .map_err(|e| {
                error!(
                    "ws remove event failed! category={}, id={}, {}",
                    self.category, self.id, e
                );
                e
            })?;

        let resp = RouterWSEventResponse::decode_string(&resp).map_err(|e| {
            error!("decode ws remove event resp failed! resp={}, {}", resp, e);
            e
        })?;

        let ret;
        if resp.err == 0 {
            info!(
                "ws remove event success! category={}, id={}",
                self.category, self.id
            );
            ret = true;
        } else {
            warn!(
                "ws remove event failed! category={}, id={}, {:?}",
                self.category, self.id, resp
            );
            ret = false;
        }

        // 只要调用成功了，那么都认为当前unregister操作完毕了
        Ok(ret)
    }
}

#[derive(Clone)]
struct RouterWSEventRequestEvent {
    owner: Arc<Mutex<RouterWSEventManagerImpl>>,
}

impl RouterWSEventRequestEvent {
    fn new(owner: Arc<Mutex<RouterWSEventManagerImpl>>) -> Self {
        Self { owner }
    }
}

#[async_trait::async_trait]
impl WebSocketRequestHandler for RouterWSEventRequestEvent {
    async fn on_string_request(
        &self,
        requestor: Arc<WebSocketRequestManager>,
        cmd: u16,
        content: String,
    ) -> BuckyResult<Option<String>> {
        match cmd {
            ROUTER_WS_EVENT_CMD_EVENT => {
                RouterWSEventManagerImpl::on_event(self.owner.clone(), content).await
            }

            _ => {
                let msg = format!("unknown ws event cmd: sid={}, cmd={}", requestor.sid(), cmd);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    async fn on_session_begin(&self, session: &Arc<WebSocketSession>) {
        let session = session.clone();
        let owner = self.owner.clone();

        RouterWSEventManagerImpl::on_session_begin(owner, session).await;
    }

    async fn on_session_end(&self, session: &Arc<WebSocketSession>) {
        let session = session.clone();
        let owner = self.owner.clone();

        RouterWSEventManagerImpl::on_session_end(owner, session).await;
    }

    fn clone_handler(&self) -> Box<dyn WebSocketRequestHandler> {
        Box::new(self.clone())
    }
}

struct RouterWSEventManagerImpl {
    events: HashMap<RouterEventId, Arc<RouterEventItem>>,

    unregister_events: HashMap<RouterEventId, Arc<RouterEventUnregisterItem>>,

    session: Option<Arc<WebSocketSession>>,
}

impl Drop for RouterWSEventManagerImpl {
    fn drop(&mut self) {
        warn!("router event manager dropped! sid={:?}", self.sid());
    }
}

impl RouterWSEventManagerImpl {
    pub fn new() -> Self {
        Self {
            events: HashMap::new(),
            unregister_events: HashMap::new(),
            session: None,
        }
    }

    pub fn sid(&self) -> Option<u32> {
        self.session.as_ref().map(|session| session.sid())
    }

    pub fn get_event(&self, id: &RouterEventId) -> Option<Arc<RouterEventItem>> {
        self.events.get(id).map(|v| v.clone())
    }

    pub fn add_event(&mut self, event_item: RouterEventItem) -> BuckyResult<()> {
        let event_item = Arc::new(event_item);

        let id = RouterEventId {
            category: event_item.category.clone(),
            id: event_item.id.clone(),
        };

        match self.events.entry(id) {
            Entry::Occupied(_) => {
                error!("router event already exists! id={}", event_item.id);
                return Err(BuckyError::from(BuckyErrorCode::AlreadyExists));
            }

            Entry::Vacant(vc) => {
                vc.insert(event_item.clone());

                if let Some(session) = &self.session {
                    let requestor = session.requestor().clone();
                    task::spawn(async move {
                        let _ = event_item.register(&requestor).await;
                    });
                }

                Ok(())
            }
        }
    }

    pub async fn remove_event(
        manager: &Arc<Mutex<RouterWSEventManagerImpl>>,
        category: RouterEventCategory,
        id: &str,
        dec_id: Option<ObjectId>,
    ) -> BuckyResult<bool> {
        let unregister_item =
            manager
                .lock()
                .unwrap()
                .remove_event_op(category.clone(), id.to_owned(), dec_id);

        let ret = manager.lock().unwrap().session.clone();
        if let Some(session) = ret {
            unregister_item.unregister(session.requestor()).await
        } else {
            let msg = format!(
                "remove ws router event but not connect: category={}, id={}",
                category, id
            );
            warn!("{}", msg);

            Err(BuckyError::new(BuckyErrorCode::NotConnected, msg))
        }
    }

    fn remove_event_op(
        &mut self,
        category: RouterEventCategory,
        id: String,
        dec_id: Option<ObjectId>,
    ) -> Arc<RouterEventUnregisterItem> {
        let id = RouterEventId {
            category: category,
            id: id,
        };

        // 首先尝试从rules里面移除，可能存在也可能不存在
        let ret = self.events.remove(&id);

        match ret {
            Some(item) => {
                assert!(item.category == id.category);
                assert!(item.id == id.id);

                info!("will remove ws router event: id={:?}, dec={:?}", id, dec_id,);
            }
            None => {
                info!(
                    "will remove ws router event without exists: id={:?}, dec={:?}",
                    id, dec_id,
                );
            }
        };

        // 添加到反注册队列等待处理
        let unregister_item = RouterEventUnregisterItem {
            category: id.category.clone(),
            id: id.id.clone(),
            dec_id,
        };

        let unregister_item = Arc::new(unregister_item);
        self.unregister_events.insert(id, unregister_item.clone());

        unregister_item
    }

    async fn on_event(
        manager: Arc<Mutex<RouterWSEventManagerImpl>>,
        content: String,
    ) -> BuckyResult<Option<String>> {
        let event = RouterWSEventEmitParam::decode_string(&content)?;

        info!(
            "on event: category={}, id={}, param={}",
            event.category, event.id, event.param
        );

        let id = RouterEventId {
            category: event.category,
            id: event.id,
        };

        let ret = manager.lock().unwrap().get_event(&id);

        if ret.is_none() {
            let msg = format!("router ws event not found! id={:?}", id);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let item = ret.unwrap();
        let resp = Self::emit(item, event.param).await?;

        Ok(Some(resp))
    }

    pub async fn emit(event: Arc<RouterEventItem>, param: String) -> BuckyResult<String> {
        // 这里回调回来，一定是存在routine注册的，所以routine为空则标识有问题
        event.routine.emit(param).await
    }

    async fn on_session_begin(
        manager: Arc<Mutex<RouterWSEventManagerImpl>>,
        session: Arc<WebSocketSession>,
    ) {
        info!("ws event session begin: sid={}", session.sid());
        {
            let mut manager = manager.lock().unwrap();
            assert!(manager.session.is_none());
            manager.session = Some(session.clone());
        }

        async_std::task::spawn(async move {
            Self::unregister_all(&manager, &session).await;

            Self::register_all(&manager, &session).await;
        });
    }

    async fn register_all(
        manager: &Arc<Mutex<RouterWSEventManagerImpl>>,
        session: &Arc<WebSocketSession>,
    ) {
        let events = manager.lock().unwrap().events.clone();
        if events.is_empty() {
            return;
        }

        let requestor = session.requestor().clone();

        // 对存在的rules执行注册
        let mut all = Vec::new();
        for (_, item) in events {
            let requestor = requestor.clone();
            all.push(task::spawn(async move {
                // TODO 这里注册失败了如何处理？
                let _ = item.register(&requestor).await;
            }));
        }

        let _ = futures::future::join_all(all).await;
    }

    async fn unregister_all(
        manager: &Arc<Mutex<RouterWSEventManagerImpl>>,
        session: &Arc<WebSocketSession>,
    ) {
        let events = manager.lock().unwrap().unregister_events.clone();
        if events.is_empty() {
            return;
        }
        let requestor = session.requestor().clone();

        // 对存在的反注册操作，批量执行反注册
        let mut all = Vec::new();
        for (id, item) in events {
            let requestor = requestor.clone();
            let manager = manager.clone();

            all.push(task::spawn(async move {
                if let Ok(_) = item.unregister(&requestor).await {
                    manager.lock().unwrap().unregister_events.remove(&id);
                }
            }));
        }

        let _ = futures::future::join_all(all).await;
    }

    async fn on_session_end(
        manager: Arc<Mutex<RouterWSEventManagerImpl>>,
        session: Arc<WebSocketSession>,
    ) {
        info!("ws event session end: sid={}", session.sid());

        {
            let mut manager = manager.lock().unwrap();
            assert!(manager.session.is_some());
            manager.session = None;
        }
    }
}

#[derive(Clone)]
pub(crate) struct RouterWSEventManager {
    manager: Arc<Mutex<RouterWSEventManagerImpl>>,
    client: WebSocketClient,
}

impl RouterWSEventManager {
    // service_url: cyfs-stack rules服务地址
    pub fn new(service_url: Url) -> Self {
        let manager = Arc::new(Mutex::new(RouterWSEventManagerImpl::new()));

        let event = RouterWSEventRequestEvent::new(manager.clone());

        let client = WebSocketClient::new(service_url, Box::new(event));

        let ret = Self { manager, client };

        ret
    }

    pub fn start(&self) {
        self.client.start();
    }

    pub async fn stop(&self) {
        let sid = self.manager.lock().unwrap().sid();
        info!("will stop event manager! sid={:?}", sid);

        self.client.stop().await;

        info!("stop event manager complete! sid={:?}", sid);

        // assert!(self.manager.lock().unwrap().session.is_none());
    }

    pub fn add_event<REQ, RESP>(
        &self,
        id: &str,
        dec_id: Option<ObjectId>,
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
        let category = extract_router_event_category::<RouterEventRequest<REQ>>();

        let routine = RouterEventRoutineT::<REQ, RESP>(routine);

        let event_item = RouterEventItem {
            category,
            id: id.to_owned(),
            dec_id,
            index,
            routine: Box::new(routine),
        };

        info!(
            "will add event: category={}, id={}, dec={:?}, index={}",
            event_item.category, event_item.id, event_item.dec_id, event_item.index
        );

        self.manager.lock().unwrap().add_event(event_item)
    }

    pub async fn remove_event(
        &self,
        category: RouterEventCategory,
        id: &str,
        dec_id: Option<ObjectId>,
    ) -> BuckyResult<bool> {
        info!(
            "will remove event: category={}, id={}, dec={:?},",
            category, id, dec_id,
        );

        RouterWSEventManagerImpl::remove_event(&self.manager, category, id, dec_id).await
    }
}
