use super::super::RouterHandlerId;
use super::super::*;
use crate::base::*;
use crate::ws::*;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_util::*;

use async_std::task;
use async_trait::async_trait;
use http_types::Url;
use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::sync::Arc;

struct RouterHandlerItem {
    chain: RouterHandlerChain,
    category: RouterHandlerCategory,
    id: String,
    dec_id: Option<ObjectId>,
    index: i32,
    filter: Option<String>,
    req_path: Option<String>,
    default_action: RouterHandlerAction,
    routine: Option<Box<dyn RouterHandlerAnyRoutine>>,
}

impl RouterHandlerItem {
    async fn emit(&self, param: String) -> BuckyResult<String> {
        if self.routine.is_none() {
            error!(
                "emit router handler event but routine is none! id={}",
                self.id
            );

            return Ok(RouterHandlerResponseHelper::encode_with_action(
                self.default_action.clone(),
            ));
        }

        let routine = self.routine.as_ref().unwrap();
        routine.emit(param).await
    }

    async fn register(&self, requestor: &Arc<WebSocketRequestManager>) -> BuckyResult<()> {
        info!(
            "will add ws router handler: chain={}, category={}, id={}, sid={}, routine={}",
            self.id,
            self.chain,
            self.category,
            requestor.sid(),
            self.routine.is_some()
        );

        let mut param = RouterAddHandlerParam {
            filter: self.filter.clone(),
            req_path: self.req_path.clone(),
            index: self.index,
            default_action: self.default_action.clone(),
            routine: None,
        };

        if self.routine.is_some() {
            param.routine = Some(requestor.sid().to_string());
        }

        let req = RouterWSAddHandlerParam {
            chain: self.chain.clone(),
            category: self.category.clone(),
            id: self.id.clone(),
            dec_id: self.dec_id.clone(),
            param,
        };

        let msg = req.encode_string();
        let resp = requestor
            .post_req(ROUTER_WS_HANDLER_CMD_ADD, msg)
            .await
            .map_err(|e| {
                error!(
                    "ws add handler failed! chain={}, category={}, id={}, {}",
                    req.chain, req.category, req.id, e
                );
                e
            })?;

        let resp = RouterWSHandlerResponse::decode_string(&resp).map_err(|e| {
            error!(
                "decode add ws router handler resp failed! resp={}, {}",
                resp, e
            );
            e
        })?;

        if resp.err == 0 {
            info!(
                "add ws router handler success: chain={}, category={}, id={}, dec={:?}",
                req.chain, req.category, req.id, req.dec_id,
            );
            Ok(())
        } else {
            error!(
                "add ws router handler failed! chain={}, category={}, id={}, dec={:?}, err={}, msg={:?}",
                req.chain, req.category, req.id, req.dec_id, resp.err, resp.msg
            );

            Err(BuckyError::new(resp.err, resp.msg.unwrap_or("".to_owned())))
        }
    }
}

struct RouterHandlerUnregisterItem {
    chain: RouterHandlerChain,
    category: RouterHandlerCategory,
    id: String,
    dec_id: Option<ObjectId>,
}

impl RouterHandlerUnregisterItem {
    async fn unregister(&self, requestor: &Arc<WebSocketRequestManager>) -> BuckyResult<bool> {
        info!(
            "ws will remove handler: chain={}, category={}, id={}, dec={:?}, sid={}",
            self.chain,
            self.category,
            self.id,
            self.dec_id,
            requestor.sid()
        );

        let req = RouterWSRemoveHandlerParam {
            chain: self.chain.clone(),
            category: self.category.clone(),
            id: self.id.clone(),
            dec_id: self.dec_id.clone(),
        };

        let msg = req.encode_string();
        let resp = requestor
            .post_req(ROUTER_WS_HANDLER_CMD_REMOVE, msg)
            .await
            .map_err(|e| {
                error!(
                    "ws remove handler failed! chain={}, category={}, id={}, {}",
                    self.chain, self.category, self.id, e
                );
                e
            })?;

        let resp = RouterWSHandlerResponse::decode_string(&resp).map_err(|e| {
            error!("decode ws remove handler resp failed! resp={}, {}", resp, e);
            e
        })?;

        let ret;
        if resp.err == 0 {
            info!(
                "ws remove handler success! chain={}, category={}, id={}, dec={:?}",
                self.chain, self.category, self.id, self.dec_id,
            );
            ret = true;
        } else {
            warn!(
                "ws remove handler failed! chain={}, category={}, id={}, dec={:?}, {:?}",
                self.chain, self.category, self.id, self.dec_id, resp
            );
            ret = false;
        }

        // 只要调用成功了，那么都认为当前unregister操作完毕了
        Ok(ret)
    }
}

#[derive(Clone)]
struct RouterWSHandlerRequestHandler {
    owner: Arc<Mutex<RouterWSHandlerManagerImpl>>,
}

impl RouterWSHandlerRequestHandler {
    fn new(owner: Arc<Mutex<RouterWSHandlerManagerImpl>>) -> Self {
        Self { owner }
    }
}

#[async_trait]
impl WebSocketRequestHandler for RouterWSHandlerRequestHandler {
    async fn on_string_request(
        &self,
        requestor: Arc<WebSocketRequestManager>,
        cmd: u16,
        content: String,
    ) -> BuckyResult<Option<String>> {
        match cmd {
            ROUTER_WS_HANDLER_CMD_EVENT => {
                RouterWSHandlerManagerImpl::on_event(self.owner.clone(), content).await
            }

            _ => {
                let msg = format!(
                    "unknown ws handler cmd: sid={}, cmd={}",
                    requestor.sid(),
                    cmd
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    async fn on_session_begin(&self, session: &Arc<WebSocketSession>) {
        let session = session.clone();
        let owner = self.owner.clone();

        RouterWSHandlerManagerImpl::on_session_begin(owner, session).await;
    }

    async fn on_session_end(&self, session: &Arc<WebSocketSession>) {
        let session = session.clone();
        let owner = self.owner.clone();

        RouterWSHandlerManagerImpl::on_session_end(owner, session).await;
    }

    fn clone_handler(&self) -> Box<dyn WebSocketRequestHandler> {
        Box::new(self.clone())
    }
}

struct RouterWSHandlerManagerImpl {
    handlers: HashMap<RouterHandlerId, Arc<RouterHandlerItem>>,

    unregister_handlers: HashMap<RouterHandlerId, Arc<RouterHandlerUnregisterItem>>,

    session: Option<Arc<WebSocketSession>>,
}

impl Drop for RouterWSHandlerManagerImpl {
    fn drop(&mut self) {
        warn!("router handler manager dropped! sid={:?}", self.sid());
    }
}

impl RouterWSHandlerManagerImpl {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            unregister_handlers: HashMap::new(),
            session: None,
        }
    }

    pub fn sid(&self) -> Option<u32> {
        self.session.as_ref().map(|session| session.sid())
    }

    pub fn get_handler(&self, id: &RouterHandlerId) -> Option<Arc<RouterHandlerItem>> {
        self.handlers.get(id).map(|v| v.clone())
    }

    pub fn add_handler(&mut self, handler_item: RouterHandlerItem) -> BuckyResult<()> {
        let handler_item = Arc::new(handler_item);

        let id = RouterHandlerId {
            chain: handler_item.chain.clone(),
            category: handler_item.category.clone(),
            id: handler_item.id.clone(),
        };

        match self.handlers.entry(id) {
            Entry::Occupied(_) => {
                error!("router handler already exists! id={}", handler_item.id);
                return Err(BuckyError::from(BuckyErrorCode::AlreadyExists));
            }

            Entry::Vacant(vc) => {
                vc.insert(handler_item.clone());

                if let Some(session) = &self.session {
                    let requestor = session.requestor().clone();
                    task::spawn(async move {
                        let _ = handler_item.register(&requestor).await;
                    });
                } else {
                    warn!(
                        "add handler but ws session does not exist yet, now will pending! id={}",
                        handler_item.id
                    );
                }

                Ok(())
            }
        }
    }

    pub async fn remove_handler(
        manager: &Arc<Mutex<RouterWSHandlerManagerImpl>>,
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: &str,
        dec_id: Option<ObjectId>,
    ) -> BuckyResult<bool> {
        let unregister_item = manager.lock().unwrap().remove_handler_op(
            chain.clone(),
            category.clone(),
            id.to_owned(),
            dec_id,
        );

        let ret = manager.lock().unwrap().session.clone();
        if let Some(session) = ret {
            unregister_item.unregister(session.requestor()).await
        } else {
            let msg = format!(
                "remove ws router handler but not connect: chain={}, category={}, id={}",
                chain, category, id
            );
            warn!("{}", msg);

            Err(BuckyError::new(BuckyErrorCode::NotConnected, msg))
        }
    }

    fn remove_handler_op(
        &mut self,
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: String,
        dec_id: Option<ObjectId>,
    ) -> Arc<RouterHandlerUnregisterItem> {
        let id = RouterHandlerId {
            chain: chain,
            category: category,
            id: id,
        };

        // 首先尝试从rules里面移除，可能存在也可能不存在
        let ret = self.handlers.remove(&id);

        match ret {
            Some(item) => {
                assert!(item.category == id.category);
                assert!(item.id == id.id);

                info!("will remove ws router handler: id={:?}", id);
            }
            None => {
                info!("will remove ws router handler without exists: id={:?}", id);
            }
        };

        // 添加到反注册队列等待处理
        let unregister_item = RouterHandlerUnregisterItem {
            chain: id.chain.clone(),
            category: id.category.clone(),
            id: id.id.clone(),
            dec_id,
        };

        let unregister_item = Arc::new(unregister_item);
        self.unregister_handlers.insert(id, unregister_item.clone());

        unregister_item
    }

    async fn on_event(
        manager: Arc<Mutex<RouterWSHandlerManagerImpl>>,
        content: String,
    ) -> BuckyResult<Option<String>> {
        let event = RouterWSHandlerEventParam::decode_string(&content)?;

        let id = RouterHandlerId {
            chain: event.chain,
            category: event.category,
            id: event.id,
        };

        let ret = manager.lock().unwrap().get_handler(&id);

        if ret.is_none() {
            let msg = format!("router ws handler not found! id={:?}", id);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let handler = ret.unwrap();
        let resp = Self::emit(handler, event.param).await?;

        Ok(Some(resp))
    }

    pub async fn emit(handler: Arc<RouterHandlerItem>, param: String) -> BuckyResult<String> {
        // 这里回调回来，一定是存在routine注册的，所以routine为空则标识有问题
        if handler.routine.is_none() {
            let msg = format!("router handler routine is emtpy! id={}", handler.id);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        handler.routine.as_ref().unwrap().emit(param).await
    }

    async fn on_session_begin(
        manager: Arc<Mutex<RouterWSHandlerManagerImpl>>,
        session: Arc<WebSocketSession>,
    ) {
        info!("ws handler session begin: sid={}", session.sid());
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
        manager: &Arc<Mutex<RouterWSHandlerManagerImpl>>,
        session: &Arc<WebSocketSession>,
    ) {
        let handlers = manager.lock().unwrap().handlers.clone();
        if handlers.is_empty() {
            return;
        }

        let requestor = session.requestor().clone();

        // 对存在的rules执行注册
        let mut all = Vec::new();
        for (_, item) in handlers {
            let requestor = requestor.clone();
            all.push(task::spawn(async move {
                // TODO 这里注册失败了如何处理？
                let _ = item.register(&requestor).await;
            }));
        }

        let _ = futures::future::join_all(all).await;
    }

    async fn unregister_all(
        manager: &Arc<Mutex<RouterWSHandlerManagerImpl>>,
        session: &Arc<WebSocketSession>,
    ) {
        let handlers = manager.lock().unwrap().unregister_handlers.clone();
        if handlers.is_empty() {
            return;
        }
        let requestor = session.requestor().clone();

        // 对存在的反注册操作，批量执行反注册
        let mut all = Vec::new();
        for (id, item) in handlers {
            let requestor = requestor.clone();
            let manager = manager.clone();

            all.push(task::spawn(async move {
                if let Ok(_) = item.unregister(&requestor).await {
                    manager.lock().unwrap().unregister_handlers.remove(&id);
                }
            }));
        }

        let _ = futures::future::join_all(all).await;
    }

    async fn on_session_end(
        manager: Arc<Mutex<RouterWSHandlerManagerImpl>>,
        session: Arc<WebSocketSession>,
    ) {
        info!("ws handler session end: sid={}", session.sid());

        {
            let mut manager = manager.lock().unwrap();
            assert!(manager.session.is_some());
            manager.session = None;
        }
    }
}

#[derive(Clone)]
pub(crate) struct RouterWSHandlerManager {
    manager: Arc<Mutex<RouterWSHandlerManagerImpl>>,
    client: WebSocketClient,
}

impl RouterWSHandlerManager {
    // service_url: cyfs-stack rules服务地址
    pub fn new(service_url: Url) -> Self {
        let manager = Arc::new(Mutex::new(RouterWSHandlerManagerImpl::new()));

        let handler = RouterWSHandlerRequestHandler::new(manager.clone());
        let client = WebSocketClient::new(service_url, Box::new(handler));

        Self { manager, client }
    }

    pub fn start(&self) {
        self.client.start();
    }

    pub async fn stop(&self) {
        info!("will stop router handler manager! sid={:?}", self.manager.lock().unwrap().sid());

        self.client.stop().await;
        // assert!(self.manager.lock().unwrap().session.is_none());
    }

    pub fn add_handler<REQ, RESP>(
        &self,
        chain: RouterHandlerChain,
        id: &str,
        dec_id: Option<ObjectId>,
        index: i32,
        filter: Option<String>,
        req_path: Option<String>,
        default_action: RouterHandlerAction,
        routine: Option<
            Box<
                dyn EventListenerAsyncRoutine<
                    RouterHandlerRequest<REQ, RESP>,
                    RouterHandlerResponse<REQ, RESP>,
                >,
            >,
        >,
    ) -> BuckyResult<()>
    where
        REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
        RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
        RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
    {
        let category = extract_router_handler_category::<RouterHandlerRequest<REQ, RESP>>();

        let mut handler_item = RouterHandlerItem {
            chain,
            category,
            id: id.to_owned(),
            dec_id,
            index,
            filter: filter.to_owned(),
            req_path,
            default_action: default_action.clone(),
            routine: None,
        };

        if let Some(routine) = routine {
            let routine = RouterHandlerRoutineT::<REQ, RESP>(routine);
            handler_item.routine = Some(Box::new(routine));
        }

        self.manager.lock().unwrap().add_handler(handler_item)
    }

    pub async fn remove_handler(
        &self,
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: &str,
        dec_id: Option<ObjectId>,
    ) -> BuckyResult<bool> {
        RouterWSHandlerManagerImpl::remove_handler(&self.manager, chain, category, id, dec_id).await
    }
}
