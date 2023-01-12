use crate::base::*;
//use crate::file::*;
//use crate::raw::*;
use super::uni_stack::*;
use crate::crypto::*;
use crate::events::*;
use crate::ndn::*;
use crate::non::*;
use crate::rmeta::*;
use crate::root_state::*;
use crate::router_handler::*;
use crate::storage::StateStorage;
use crate::sync::*;
use crate::trans::*;
use crate::util::*;
use cyfs_base::*;

use http_types::Url;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use std::sync::RwLock;

pub type SharedObjectStackDecID = Arc<OnceCell<ObjectId>>;

struct CyfsStackProcessors {
    pub non_service: NONOutputProcessorRef,
    pub ndn_service: NDNOutputProcessorRef,
    pub crypto_service: CryptoOutputProcessorRef,
    pub util_service: UtilOutputProcessorRef,
    pub trans_service: TransOutputProcessorRef,

    pub router_handlers: RouterHandlerManagerProcessorRef,
    pub router_events: RouterEventManagerProcessorRef,

    pub root_state: GlobalStateOutputProcessorRef,
    pub root_state_accessor: GlobalStateAccessorOutputProcessorRef,

    pub local_cache: GlobalStateOutputProcessorRef,
    pub local_cache_accessor: GlobalStateAccessorOutputProcessorRef,

    pub root_state_meta: GlobalStateMetaOutputProcessorRef,
    pub local_cache_meta: GlobalStateMetaOutputProcessorRef,
}

impl Drop for CyfsStackProcessors {
    fn drop(&mut self) {
        warn!("CyfsStackProcessors dropped!");
    }
}

pub(crate) struct ObjectServices {
    non_service: NONRequestor,
    ndn_service: NDNRequestor,

    crypto_service: CryptoRequestor,

    util_service: UtilRequestor,
    trans_service: TransRequestor,
    sync_service: SyncRequestor,

    root_state: GlobalStateRequestor,
    root_state_accessor: GlobalStateAccessorRequestor,

    local_cache: GlobalStateRequestor,
    local_cache_accessor: GlobalStateAccessorRequestor,

    root_state_meta: GlobalStateMetaRequestor,
    local_cache_meta: GlobalStateMetaRequestor,
}

impl Drop for ObjectServices {
    fn drop(&mut self) {
        warn!("object services dropped!");
    }
}

#[derive(Clone)]
pub struct SharedCyfsStack {
    param: SharedCyfsStackParam,

    // 所属的dec_id
    dec_id: SharedObjectStackDecID,

    services: Arc<ObjectServices>,
    processors: Arc<CyfsStackProcessors>,

    // router handler
    router_handlers: RouterHandlerManager,

    // router events
    router_events: RouterEventManager,

    // 当前协议栈的device
    device_info: Arc<RwLock<Option<(DeviceId, Device)>>>,

    // uni_stack
    uni_stack: Arc<OnceCell<UniCyfsStackRef>>,

    requestor_holder: RequestorHolder,
}

#[derive(Debug, Clone)]
pub enum CyfsStackEventType {
    WebSocket(Url),
}

#[derive(Debug, Clone)]
pub enum CyfsStackRequestorType {
    Http,
    WebSocket,
}

#[derive(Debug, Clone)]
pub struct CyfsStackRequestorConfig {
    pub non_service: CyfsStackRequestorType,
    pub ndn_service: CyfsStackRequestorType,
    pub util_service: CyfsStackRequestorType,
    pub trans_service: CyfsStackRequestorType,
    pub crypto_service: CyfsStackRequestorType,
    pub root_state: CyfsStackRequestorType,
    pub local_cache: CyfsStackRequestorType,
}

impl CyfsStackRequestorConfig {
    pub fn default() -> Self {
        Self {
            non_service: CyfsStackRequestorType::Http,
            ndn_service: CyfsStackRequestorType::Http,
            util_service: CyfsStackRequestorType::Http,
            trans_service: CyfsStackRequestorType::Http,
            crypto_service: CyfsStackRequestorType::Http,
            root_state: CyfsStackRequestorType::Http,
            local_cache: CyfsStackRequestorType::Http,
        }
    }

    pub fn ws() -> Self {
        Self {
            non_service: CyfsStackRequestorType::WebSocket,
            ndn_service: CyfsStackRequestorType::WebSocket,
            util_service: CyfsStackRequestorType::WebSocket,
            trans_service: CyfsStackRequestorType::WebSocket,
            crypto_service: CyfsStackRequestorType::WebSocket,
            root_state: CyfsStackRequestorType::WebSocket,
            local_cache: CyfsStackRequestorType::WebSocket,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SharedCyfsStackParam {
    pub dec_id: Option<ObjectId>,

    // 基于http协议的服务地址
    pub service_url: Url,

    // 基于websocket协议的服务地址
    pub ws_url: Url,

    pub event_type: CyfsStackEventType,

    pub requestor_config: CyfsStackRequestorConfig,
}

impl SharedCyfsStackParam {
    fn gen_url(http_port: u16, ws_port: u16) -> (Url, Url) {
        assert_ne!(http_port, ws_port);

        let service_url = format!("http://127.0.0.1:{}", http_port).parse().unwrap();
        let ws_url = format!("ws://127.0.0.1:{}", ws_port).parse().unwrap();

        (service_url, ws_url)
    }

    pub fn default_with_http_event(dec_id: Option<ObjectId>) -> Self {
        Self::default(dec_id)
    }

    // 默认切换到websocket模式
    pub fn default(dec_id: Option<ObjectId>) -> Self {
        let (service_url, ws_url) =
            Self::gen_url(cyfs_base::NON_STACK_HTTP_PORT, cyfs_base::NON_STACK_WS_PORT);

        Self {
            dec_id,
            service_url,
            event_type: CyfsStackEventType::WebSocket(ws_url.clone()),
            ws_url,
            requestor_config: CyfsStackRequestorConfig::default(),
        }
    }

    // 提供给cyfs-runtime使用的shareobjectstack
    pub fn default_runtime(dec_id: Option<ObjectId>) -> Self {
        let (service_url, ws_url) = Self::gen_url(
            cyfs_base::CYFS_RUNTIME_NON_STACK_HTTP_PORT,
            cyfs_base::CYFS_RUNTIME_NON_STACK_WS_PORT,
        );

        Self {
            dec_id,
            service_url,
            event_type: CyfsStackEventType::WebSocket(ws_url.clone()),
            ws_url,
            requestor_config: CyfsStackRequestorConfig::default(),
        }
    }

    // 打开指定端口的shareobjectstack
    pub fn gen(dec_id: Option<ObjectId>, http_port: u16, ws_port: u16) -> Self {
        let (service_url, ws_url) = Self::gen_url(http_port, ws_port);

        Self {
            dec_id,
            service_url,
            event_type: CyfsStackEventType::WebSocket(ws_url.clone()),
            ws_url,
            requestor_config: CyfsStackRequestorConfig::default(),
        }
    }

    pub fn new_with_ws_event(
        dec_id: Option<ObjectId>,
        service_url: &str,
        ws_url: &str,
    ) -> BuckyResult<Self> {
        let service_url = Url::parse(service_url).map_err(|e| {
            let msg = format!("invalid http service url! url={}, {}", service_url, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let ws_url = Url::parse(ws_url).map_err(|e| {
            let msg = format!("invalid ws service url! url={}, {}", ws_url, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(Self {
            dec_id,
            service_url,
            event_type: CyfsStackEventType::WebSocket(ws_url.clone()),
            ws_url,
            requestor_config: CyfsStackRequestorConfig::default(),
        })
    }
}

#[derive(Clone)]
struct RequestorHolder {
    http: Option<HttpRequestorRef>,
    ws: Option<HttpRequestorRef>,
}

impl RequestorHolder {
    fn new() -> Self {
        Self {
            http: None,
            ws: None,
        }
    }

    fn select_requestor(
        &mut self,
        param: &SharedCyfsStackParam,
        requestor_type: &CyfsStackRequestorType,
    ) -> HttpRequestorRef {
        match *requestor_type {
            CyfsStackRequestorType::Http => {
                self.http
                    .get_or_insert_with(|| {
                        // 基于标准http的requestor
                        let addr = format!(
                            "{}:{}",
                            param.service_url.host_str().unwrap(),
                            param.service_url.port().unwrap()
                        );
                        Arc::new(Box::new(TcpHttpRequestor::new(&addr)))
                    })
                    .clone()
            }
            CyfsStackRequestorType::WebSocket => {
                self.ws
                    .get_or_insert_with(|| {
                        // 基于websocket协议的requestor
                        Arc::new(Box::new(WSHttpRequestor::new(param.ws_url.clone())))
                    })
                    .clone()
            }
        }
    }

    async fn stop(&self) {
        if let Some(requestor) = &self.http {
            requestor.stop().await;
        }
        if let Some(requestor) = &self.ws {
            requestor.stop().await;
        }
    }
}

impl SharedCyfsStack {
    pub async fn open_default(dec_id: Option<ObjectId>) -> BuckyResult<Self> {
        Self::open(SharedCyfsStackParam::default(dec_id)).await
    }

    pub async fn open_default_with_ws_event(dec_id: Option<ObjectId>) -> BuckyResult<Self> {
        Self::open(SharedCyfsStackParam::default(dec_id)).await
    }

    pub async fn open_runtime(dec_id: Option<ObjectId>) -> BuckyResult<Self> {
        Self::open(SharedCyfsStackParam::default_runtime(dec_id)).await
    }

    pub async fn open_with_port(
        dec_id: Option<ObjectId>,
        http_port: u16,
        ws_port: u16,
    ) -> BuckyResult<Self> {
        Self::open(SharedCyfsStackParam::gen(dec_id, http_port, ws_port)).await
    }

    pub async fn open(param: SharedCyfsStackParam) -> BuckyResult<Self> {
        Self::sync_open(param)
    }
    
    pub fn sync_open(param: SharedCyfsStackParam) -> BuckyResult<Self> {
        info!("will init shared object stack: {:?}", param);

        let dec_id = Arc::new(OnceCell::new());
        if let Some(id) = &param.dec_id {
            dec_id.set(id.clone()).unwrap();
        }

        let mut requestor_holder = RequestorHolder::new();

        // trans service
        let requestor =
            requestor_holder.select_requestor(&param, &param.requestor_config.trans_service);
        let trans_service = TransRequestor::new(Some(dec_id.clone()), requestor);

        // util
        let requestor =
            requestor_holder.select_requestor(&param, &param.requestor_config.util_service);
        let util_service = UtilRequestor::new(Some(dec_id.clone()), requestor);

        // crypto
        let requestor =
            requestor_holder.select_requestor(&param, &param.requestor_config.crypto_service);
        let crypto_service = CryptoRequestor::new(Some(dec_id.clone()), requestor);

        // non
        let requestor =
            requestor_holder.select_requestor(&param, &param.requestor_config.non_service);
        let non_service = NONRequestor::new(Some(dec_id.clone()), requestor);

        // ndn
        let requestor =
            requestor_holder.select_requestor(&param, &param.requestor_config.ndn_service);
        let ndn_service = NDNRequestor::new(Some(dec_id.clone()), requestor);

        // sync
        let requestor = requestor_holder.select_requestor(&param, &CyfsStackRequestorType::Http);
        let sync_service = SyncRequestor::new(requestor);

        // root_state
        let requestor =
            requestor_holder.select_requestor(&param, &param.requestor_config.root_state);
        let root_state =
            GlobalStateRequestor::new_root_state(Some(dec_id.clone()), requestor.clone());

        let root_state_accessor =
            GlobalStateAccessorRequestor::new_root_state(Some(dec_id.clone()), requestor.clone());

        let root_state_meta =
            GlobalStateMetaRequestor::new_root_state(Some(dec_id.clone()), requestor.clone());

        // local_cache
        let requestor =
            requestor_holder.select_requestor(&param, &param.requestor_config.local_cache);
        let local_cache =
            GlobalStateRequestor::new_local_cache(Some(dec_id.clone()), requestor.clone());

        let local_cache_accessor =
            GlobalStateAccessorRequestor::new_local_cache(Some(dec_id.clone()), requestor.clone());

        let local_cache_meta =
            GlobalStateMetaRequestor::new_local_cache(Some(dec_id.clone()), requestor);

        let services = Arc::new(ObjectServices {
            non_service,
            ndn_service,
            crypto_service,

            util_service,
            trans_service,
            sync_service,

            root_state,
            root_state_accessor,

            local_cache,
            local_cache_accessor,

            root_state_meta,
            local_cache_meta,
        });

        // 初始化对应的事件处理器，二选一
        let router_handlers = match &param.event_type {
            CyfsStackEventType::WebSocket(ws_url) => {
                RouterHandlerManager::new(Some(dec_id.clone()), ws_url.clone())
            }
        };

        let router_events = match &param.event_type {
            CyfsStackEventType::WebSocket(ws_url) => {
                RouterEventManager::new(Some(dec_id.clone()), ws_url.clone())
            }
        };

        // 缓存所有processors，用以uni_stack直接返回使用
        let processors = Arc::new(CyfsStackProcessors {
            non_service: services.non_service.clone_processor(),
            ndn_service: services.ndn_service.clone_processor(),
            crypto_service: services.crypto_service.clone_processor(),
            util_service: services.util_service.clone_processor(),
            trans_service: services.trans_service.clone_processor(),
            router_handlers: router_handlers.clone_processor(),
            router_events: router_events.clone_processor(),
            root_state: services.root_state.clone_processor(),
            root_state_accessor: services.root_state_accessor.clone_processor(),
            local_cache: services.local_cache.clone_processor(),
            local_cache_accessor: services.local_cache_accessor.clone_processor(),
            root_state_meta: services.root_state_meta.clone_processor(),
            local_cache_meta: services.local_cache_meta.clone_processor(),
        });

        let ret = Self {
            param,
            dec_id,

            services,
            processors,

            router_handlers,
            router_events,

            device_info: Arc::new(RwLock::new(None)),
            uni_stack: Arc::new(OnceCell::new()),

            requestor_holder,
        };

        Ok(ret)
    }

    pub async fn stop(&self) {
        self.requestor_holder.stop().await;

        self.router_handlers.stop().await;

        self.router_events.stop().await;
    }
    
    pub fn param(&self) -> &SharedCyfsStackParam {
        &self.param
    }

    pub async fn fork_with_new_dec(&self, dec_id: Option<ObjectId>) -> BuckyResult<Self> {
        let mut param = self.param.clone();
        param.dec_id = dec_id;

        Self::open(param).await
    }

    // 等待协议栈上线
    pub async fn wait_online(&self, timeout: Option<std::time::Duration>) -> BuckyResult<()> {
        let this = self.clone();
        let ft = async move {
            loop {
                match this.online().await {
                    Ok(_) => break,
                    Err(e) => {
                        match e.code() {
                            BuckyErrorCode::ConnectFailed | BuckyErrorCode::Timeout => {
                                // 需要重试
                            }
                            _ => {
                                error!("stack online failed! {}", e);
                                return Err(e);
                            }
                        }
                    }
                }
                async_std::task::sleep(std::time::Duration::from_secs(5)).await;
            }
            Ok(())
        };
        if let Some(timeout) = timeout {
            match async_std::future::timeout(timeout, ft).await {
                Ok(ret) => ret,
                Err(async_std::future::TimeoutError { .. }) => {
                    let msg = format!("wait stack online timeout! dur={:?}", timeout);
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::Timeout, msg))
                }
            }
        } else {
            ft.await
        }
    }

    pub async fn online(&self) -> BuckyResult<()> {
        // 获取当前协议栈的device_id
        let req = UtilGetDeviceOutputRequest::new();
        let resp = self.services.util_service.get_device(req).await?;

        info!("got local stack device: {}", resp);

        *self.device_info.write().unwrap() = Some((resp.device_id, resp.device));

        Ok(())
    }

    // 如果初始化时候没有指定，那么可以延迟绑定一次
    pub fn bind_dec(&self, dec_id: ObjectId) {
        self.dec_id.set(dec_id).unwrap();
    }

    pub fn dec_id(&self) -> Option<&ObjectId> {
        self.dec_id.get()
    }

    // 下面两个接口必须调用onlien成功一次之后才可以调用
    pub fn local_device_id(&self) -> DeviceId {
        self.device_info.read().unwrap().as_ref().unwrap().0.clone()
    }

    pub fn local_device(&self) -> Device {
        self.device_info.read().unwrap().as_ref().unwrap().1.clone()
    }

    pub fn non_service(&self) -> &NONRequestor {
        &self.services.non_service
    }

    pub fn ndn_service(&self) -> &NDNRequestor {
        &self.services.ndn_service
    }

    pub fn crypto(&self) -> &CryptoRequestor {
        &self.services.crypto_service
    }

    pub fn util(&self) -> &UtilRequestor {
        &self.services.util_service
    }

    pub fn trans(&self) -> &TransRequestor {
        &self.services.trans_service
    }

    pub fn sync(&self) -> &SyncRequestor {
        &self.services.sync_service
    }

    pub fn router_handlers(&self) -> &RouterHandlerManager {
        &self.router_handlers
    }

    pub fn router_events(&self) -> &RouterEventManager {
        &self.router_events
    }

    // root_state 根状态管理相关接口
    pub fn root_state(&self) -> &GlobalStateRequestor {
        &self.services.root_state
    }

    pub fn root_state_stub(
        &self,
        target: Option<ObjectId>,
        target_dec_id: Option<ObjectId>,
    ) -> GlobalStateStub {
        GlobalStateStub::new(
            self.services.root_state.clone_processor(),
            target,
            target_dec_id,
        )
    }

    pub fn root_state_accessor_stub(
        &self,
        target: Option<ObjectId>,
        target_dec_id: Option<ObjectId>,
    ) -> GlobalStateAccessorStub {
        GlobalStateAccessorStub::new(
            self.services.root_state_accessor.clone_processor(),
            target,
            target_dec_id,
        )
    }

    // root_state meta
    pub fn root_state_meta(&self) -> &GlobalStateMetaRequestor {
        &self.services.root_state_meta
    }

    pub fn root_state_meta_stub(
        &self,
        target: Option<ObjectId>,
        target_dec_id: Option<ObjectId>,
    ) -> GlobalStateMetaStub {
        GlobalStateMetaStub::new(
            self.services.root_state_meta.clone_processor(),
            target,
            target_dec_id,
        )
    }

    // local_cache
    pub fn local_cache(&self) -> &GlobalStateRequestor {
        &self.services.local_cache
    }

    pub fn local_cache_stub(&self, target_dec_id: Option<ObjectId>) -> GlobalStateStub {
        GlobalStateStub::new(
            self.services.local_cache.clone_processor(),
            None,
            target_dec_id,
        )
    }

    pub fn local_cache_accessor_stub(
        &self,
        target: Option<ObjectId>,
        target_dec_id: Option<ObjectId>,
    ) -> GlobalStateAccessorStub {
        GlobalStateAccessorStub::new(
            self.services.local_cache_accessor.clone_processor(),
            target,
            target_dec_id,
        )
    }

    // local_cache meta
    pub fn local_cache_meta(&self) -> &GlobalStateMetaRequestor {
        &self.services.local_cache_meta
    }

    pub fn local_cache_meta_stub(&self, target_dec_id: Option<ObjectId>) -> GlobalStateMetaStub {
        GlobalStateMetaStub::new(
            self.services.local_cache_meta.clone_processor(),
            None,
            target_dec_id,
        )
    }

    // state_storage
    pub fn global_state_storage(
        &self,
        category: GlobalStateCategory,
        path: impl Into<String>,
        content_type: ObjectMapSimpleContentType,
    ) -> StateStorage {
        StateStorage::new_with_stack(
            self.uni_stack().clone(),
            category,
            path,
            content_type,
            None,
            self.dec_id().cloned(),
        )
    }

    pub fn global_state_storage_ex(
        &self,
        category: GlobalStateCategory,
        path: impl Into<String>,
        content_type: ObjectMapSimpleContentType,
        target: Option<ObjectId>,
        dec_id: Option<ObjectId>,
    ) -> StateStorage {
        StateStorage::new_with_stack(
            self.uni_stack().clone(),
            category,
            path,
            content_type,
            target,
            dec_id,
        )
    }

    // uni_stack相关接口
    fn create_uni_stack(&self) -> UniCyfsStackRef {
        Arc::new(self.clone())
    }

    pub fn uni_stack(&self) -> &UniCyfsStackRef {
        self.uni_stack.get_or_init(|| self.create_uni_stack())
    }
}

impl UniCyfsStack for SharedCyfsStack {
    fn non_service(&self) -> &NONOutputProcessorRef {
        &self.processors.non_service
    }

    fn ndn_service(&self) -> &NDNOutputProcessorRef {
        &self.processors.ndn_service
    }

    fn crypto_service(&self) -> &CryptoOutputProcessorRef {
        &self.processors.crypto_service
    }

    fn util_service(&self) -> &UtilOutputProcessorRef {
        &self.processors.util_service
    }

    fn trans_service(&self) -> &TransOutputProcessorRef {
        &self.processors.trans_service
    }

    fn router_handlers(&self) -> &RouterHandlerManagerProcessorRef {
        &self.processors.router_handlers
    }

    fn router_events(&self) -> &RouterEventManagerProcessorRef {
        &self.processors.router_events
    }

    fn root_state(&self) -> &GlobalStateOutputProcessorRef {
        &self.processors.root_state
    }

    fn root_state_accessor(&self) -> &GlobalStateAccessorOutputProcessorRef {
        &self.processors.root_state_accessor
    }

    fn local_cache(&self) -> &GlobalStateOutputProcessorRef {
        &self.processors.local_cache
    }

    fn local_cache_accessor(&self) -> &GlobalStateAccessorOutputProcessorRef {
        &self.processors.local_cache_accessor
    }

    fn root_state_meta(&self) -> &GlobalStateMetaOutputProcessorRef {
        &self.processors.root_state_meta
    }

    fn local_cache_meta(&self) -> &GlobalStateMetaOutputProcessorRef {
        &self.processors.local_cache_meta
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    async fn test_online() {
        let mut param = SharedCyfsStackParam::default(None);
        param.requestor_config = CyfsStackRequestorConfig::ws();

        let stack = SharedCyfsStack::open(param).await.unwrap();
        let _ = stack.wait_online(None).await;

        async_std::task::sleep(std::time::Duration::from_secs(60 * 2)).await;
    }

    #[test]
    fn test() {
        cyfs_base::init_simple_log("test-shared-object-stack", Some("trace"));

        async_std::task::block_on(async move {
            // test_online().await;
        })
    }
}
