use super::super::RouterHandlerId;
use super::super::*;
use super::*;
use crate::base::*;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_util::*;

use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;
use async_trait::async_trait;
use futures::future::{AbortHandle, Aborted};
use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::sync::Arc;
use tide::{Request, Response, Server};

struct TideEndpoint {
    server: RouterHttpHandlerManager,
}

impl TideEndpoint {
    fn new(server: RouterHttpHandlerManager) -> Self {
        Self { server }
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for TideEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: Request<State>) -> tide::Result {
        let resp = match self.server.process_request(req).await {
            Ok(resp) => resp,
            Err(e) => RequestorHelper::trans_error(e),
        };

        Ok(resp)
    }
}

struct RouterHandlerItem {
    id: String,
    dec_id: Option<ObjectId>,
    index: i32,
    filter: Option<String>,
    req_path: Option<String>,
    default_action: RouterHandlerAction,
    routine: Option<Box<dyn RouterHandlerAnyRoutine>>,

    // 注册器
    register: Option<RouterHandlerRegister>,
}

impl RouterHandlerItem {
    async fn emit(&self, param: String) -> BuckyResult<String> {
        if self.routine.is_none() {
            error!("emit router handler but routine is none!");
            return Ok(RouterHandlerResponseHelper::encode_with_action(
                self.default_action.clone(),
            ));
        }

        let routine = self.routine.as_ref().unwrap();
        routine.emit(param).await
    }
}

struct RouterHttpHandlerManagerImpl {
    listen: String,

    server: Server<()>,

    handlers: HashMap<RouterHandlerId, Arc<RouterHandlerItem>>,

    // routine的http回调路径，动态绑定本地地址后会设置此值
    routine_url: Option<String>,

    // cyfs-stack rules服务地址
    service_url: String,

    // 取消listener的运行
    canceler: Option<AbortHandle>,
    running_task: Option<async_std::task::JoinHandle<()>>,
}

impl RouterHttpHandlerManagerImpl {
    pub fn new(service_url: &str) -> Self {
        let server = Self::new_server();

        Self {
            listen: "127.0.0.1:0".to_owned(),
            server,
            handlers: HashMap::new(),
            routine_url: None,
            service_url: service_url.to_owned(),
            canceler: None,
            running_task: None,
        }
    }

    fn new_server() -> ::tide::Server<()> {
        use http_types::headers::HeaderValue;
        use tide::security::{CorsMiddleware, Origin};

        let mut server = ::tide::new();
        let cors = CorsMiddleware::new()
            .allow_methods("GET, POST".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*"))
            .allow_credentials(true);
        server.with(cors);
        server
    }

    pub fn get_handler(&self, id: &RouterHandlerId) -> Option<Arc<RouterHandlerItem>> {
        self.handlers.get(id).map(|v| v.clone())
    }

    pub async fn emit(handler: Arc<RouterHandlerItem>, param: String) -> BuckyResult<String> {
        if handler.routine.is_none() {
            let msg = format!("router handler routine is emtpy! rule_id={}", handler.id);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        handler.routine.as_ref().unwrap().emit(param).await
    }
}

#[derive(Clone)]
pub(crate) struct RouterHttpHandlerManager(Arc<Mutex<RouterHttpHandlerManagerImpl>>);

impl RouterHttpHandlerManager {
    pub fn new(service_url: &str) -> Self {
        let ret = Self(Arc::new(Mutex::new(RouterHttpHandlerManagerImpl::new(
            service_url,
        ))));

        ret.init_server();

        ret
    }

    pub async fn start(&self) -> BuckyResult<()> {
        let listen;
        {
            let inner = self.0.lock().unwrap();
            listen = inner.listen.clone();
        }

        let tcp_listener = match TcpListener::bind(&listen).await {
            Ok(v) => v,
            Err(e) => {
                let msg = format!(
                    "router handler listener bind addr failed! addr={}, err={}",
                    listen, e
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        let addr = format!("http://{}/event/", tcp_listener.local_addr()?);
        info!("object stack routine url: {}", addr);

        {
            let mut inner = self.0.lock().unwrap();
            inner.routine_url = Some(addr)
        }

        let this = self.clone();
        let (release_task, handle) = futures::future::abortable(async move {
            match this.run_inner(tcp_listener).await {
                Ok(_) => {
                    info!("router handler http listener finished!");
                }
                Err(e) => {
                    error!("router handler http listener finished with error: {}", e);
                }
            }
        });

        let task = async_std::task::spawn(async move {
            match release_task.await {
                Ok(_) => {
                    info!("router handler http listener finished!");
                }
                Err(Aborted) => {
                    info!("router handler http listener cancelled!");
                }
            }
        });

        {
            let mut inner = self.0.lock().unwrap();
            assert!(inner.canceler.is_none());
            assert!(inner.running_task.is_none());
            inner.canceler = Some(handle);
            inner.running_task = Some(task);
        }

        Ok(())
    }

    pub async fn stop(&self) {
        let (canceler, task) = {
            let mut inner = self.0.lock().unwrap();
            (inner.canceler.take(), inner.running_task.take())
        };

        if let Some(canceler) = canceler {
            info!("will stop router handler http listener!");
            canceler.abort();

            if let Some(task) = task {
                task.await;
            }
        }
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
        let mut item = RouterHandlerItem {
            id: id.to_owned(),
            dec_id,
            index,
            filter: filter.clone(),
            req_path: req_path.clone(),
            default_action: default_action.clone(),
            routine: None,
            register: None,
        };

        if let Some(routine) = routine {
            let routine = RouterHandlerRoutineT::<REQ, RESP>(routine);
            item.routine = Some(Box::new(routine));
        }

        // 如果存在回调函数，那么需要推导对应的http回调url
        let service_url = self.0.lock().unwrap().service_url.clone();
        let http_routine = if item.routine.is_some() {
            Some(self.0.lock().unwrap().routine_url.as_ref().unwrap().clone())
        } else {
            None
        };

        // 注册到non-stack的router
        let category = extract_router_handler_category::<RouterHandlerRequest<REQ, RESP>>();
        let register = RouterHandlerRegister::new(
            chain.clone(),
            category.clone(),
            id,
            dec_id,
            index,
            filter,
            req_path,
            default_action,
            http_routine,
            &service_url,
        );

        item.register = Some(register.clone());

        // 保存
        {
            let id = RouterHandlerId {
                id: id.to_owned(),
                category,
                chain,
            };

            let mut inner = self.0.lock().unwrap();

            match inner.handlers.entry(id.clone()) {
                Entry::Occupied(_) => {
                    error!("router handler already exists! id={:?}", id);
                    return Err(BuckyError::from(BuckyErrorCode::AlreadyExists));
                }
                Entry::Vacant(vc) => {
                    vc.insert(Arc::new(item));
                }
            };
        }

        // 发起注册
        register.register();

        Ok(())
    }

    pub async fn remove_handler(
        &self,
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: &str,
        dec_id: Option<ObjectId>,
    ) -> BuckyResult<bool> {
        let id = RouterHandlerId {
            id: id.to_owned(),
            category,
            chain,
        };

        let service_url;
        let ret = {
            let mut inner = self.0.lock().unwrap();
            service_url = inner.service_url.clone();
            inner.handlers.remove(&id)
        };

        match ret {
            Some(item) => {
                info!("will remove router handler and stop register: id={:?}", id);
                assert!(item.register.is_some());
                item.register.as_ref().unwrap().unregister().await
            }
            None => {
                info!(
                    "will remove router handler without current register: id={:?}",
                    id
                );
                let unregister = RouterHandlerUnregister::new(
                    id.chain,
                    id.category,
                    id.id,
                    dec_id,
                    &service_url,
                );
                unregister.unregister().await
            }
        }
    }

    fn init_server(&self) {
        let mut inner = self.0.lock().unwrap();

        inner
            .server
            .at("/event/:handler_chain/:handler_category/:rule_id")
            .post(TideEndpoint::new(self.clone()));
        inner
            .server
            .at("/event/:handler_chain/:handler_category/:rule_id/")
            .post(TideEndpoint::new(self.clone()));
    }

    fn extract_id_from_path<State>(
        req: &Request<State>,
    ) -> BuckyResult<(RouterHandlerChain, RouterHandlerCategory, String)> {
        // 提取路径上的rule_category+rule_id
        let handler_chain: RouterHandlerChain = req
            .param("handler_chain")
            .map_err(|e| {
                let msg = format!("invalid handler_chain: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?
            .parse()?;

        let handler_category: RouterHandlerCategory = req
            .param("handler_category")
            .map_err(|e| {
                let msg = format!("invalid handler_category: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?
            .parse()?;

        let rule_id: String = req
            .param("rule_id")
            .map_err(|e| {
                let msg = format!("invalid rule_id: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?
            .to_owned();

        Ok((handler_chain, handler_category, rule_id))
    }

    async fn process_request<State>(&self, mut req: Request<State>) -> BuckyResult<Response>
    where
        State: Clone + Send + Sync + 'static,
    {
        let (chain, category, id) = Self::extract_id_from_path(&req)?;

        match req.body_string().await {
            Ok(body) => self.emit(chain, category, id, body).await,
            Err(e) => {
                let msg = format!("read router event body error! id={}, err={}", id, e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    async fn emit(
        &self,
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: String,
        param: String,
    ) -> BuckyResult<Response> {
        let id = RouterHandlerId {
            id,
            category,
            chain,
        };

        let handler = {
            let inner = self.0.lock().unwrap();
            inner.get_handler(&id)
        };

        if handler.is_none() {
            let msg = format!("router event not found! id={:?}", id);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let handler = handler.unwrap();
        let resp = RouterHttpHandlerManagerImpl::emit(handler, param).await?;

        // 转换为tide::Response来应答
        let mut http_resp: Response = RequestorHelper::new_ok_response();
        http_resp.set_body(resp);

        Ok(http_resp)
    }

    async fn run_inner(self, tcp_listener: TcpListener) -> BuckyResult<()> {
        let addr;
        let server;
        {
            let inner = self.0.lock().unwrap();
            addr = inner.routine_url.as_ref().unwrap().clone();
            server = inner.server.clone();
        }

        let mut incoming = tcp_listener.incoming();
        loop {
            match incoming.next().await {
                Some(Ok(tcp_stream)) => {
                    debug!(
                        "router handler http listener recv new connection from {:?}",
                        tcp_stream.peer_addr()
                    );

                    let addr = addr.clone();
                    let server = server.clone();
                    task::spawn(async move {
                        if let Err(e) = Self::accept(&server, addr, tcp_stream).await {
                            error!(
                                "router handler http listener process connection error: err={}",
                                e
                            );
                        }
                    });
                }
                Some(Err(e)) => {
                    // FIXME 返回错误后如何处理？是否要停止
                    error!(
                        "recv request from router handler http listener error! addr={}, err={}",
                        addr, e
                    );
                }
                None => {
                    break;
                }
            }
        }

        Ok(())
    }

    async fn accept(server: &Server<()>, addr: String, stream: TcpStream) -> BuckyResult<()> {
        let peer_addr = stream.peer_addr()?;
        trace!(
            "router handler http listener starting accept new connection at {} from {}",
            addr,
            &peer_addr
        );

        // 一条连接上只accept一次
        let opts = async_h1::ServerOptions::default();
        let ret = async_h1::accept_with_opts(
            stream.clone(),
            |mut req| async move {
                info!(
                    "router handler http listener recv tcp request: {:?}, len={:?}",
                    req,
                    req.len()
                );

                // 用户自己的请求不可附带CYFS_REMOTE_DEVICE，避免被攻击
                req.remove_header(cyfs_base::CYFS_REMOTE_DEVICE);

                server.respond(req).await
            },
            opts,
        )
        .await;

        match ret {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!(
                    "router handler http listener accept error, addr={}, peer={}, err={}",
                    addr, peer_addr, e
                );

                // FIXME 一般是请求方直接断开导致的错误，是否需要判断并不再输出warn？
                //Err(BuckyError::from(e))
                Ok(())
            }
        }
    }
}
