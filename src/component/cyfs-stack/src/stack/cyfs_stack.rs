// use super::dsg::{DSGService, DSGServiceOptions};
use super::params::*;
use crate::acl::{AclManager, AclManagerRef};
use crate::admin::AdminManager;
use crate::app::{AppController, AppService};
use crate::config::*;
use crate::crypto::CryptoOutputTransformer;
use crate::crypto_api::{CryptoService, ObjectCrypto, ObjectVerifier};
use crate::events::RouterEventsManager;
use crate::forward::ForwardProcessorManager;
use crate::front::FrontService;
use crate::interface::{
    ObjectListenerManager, ObjectListenerManagerParams, ObjectListenerManagerRef,
};
use crate::meta::*;
use crate::name::NameResolver;
use crate::ndn::NDNOutputTransformer;
use crate::ndn_api::{ChunkStoreReader, NDNService, BdtNdnEventHandler};
use crate::non::NONOutputTransformer;
use crate::non_api::NONService;
use crate::resolver::{CompoundObjectSearcher, DeviceInfoManager, OodResolver};
use crate::root_state::{GlobalStateAccessOutputTransformer, GlobalStateOutputTransformer};
use crate::root_state_api::{GlobalStateLocalService, GlobalStateService};
use crate::router_handler::RouterHandlersManager;
use crate::trans::TransOutputTransformer;
use crate::trans_api::{create_trans_store, TransService};
use crate::util::UtilOutputTransformer;
use crate::util_api::UtilService;
use crate::zone::{ZoneManager, ZoneRoleManager};
use cyfs_base::*;
use cyfs_bdt::{
    ChunkReader, 
    DeviceCache, 
    Stack, 
    StackGuard, 
    StackOpenParams
};
use cyfs_chunk_cache::ChunkManager;
use cyfs_lib::*;
use cyfs_noc::*;
use cyfs_util::*;
use cyfs_task_manager::{SQLiteTaskStore, TaskManager};

use once_cell::sync::OnceCell;
use std::sync::Arc;

// 用来增加一些已知对象到本地noc
#[derive(Clone)]
pub struct KnownObject {
    // 对象内容
    pub object_id: ObjectId,

    pub object_raw: Vec<u8>,
    pub object: Arc<AnyNamedObject>,
}

struct CyfsStackProcessors {
    pub non_service: NONOutputProcessorRef,
    pub ndn_service: NDNOutputProcessorRef,
    pub crypto_service: CryptoOutputProcessorRef,
    pub util_service: UtilOutputProcessorRef,
    pub trans_service: TransOutputProcessorRef,

    pub router_handlers: RouterHandlerManagerProcessorRef,
    pub router_events: RouterEventManagerProcessorRef,

    pub root_state: GlobalStateOutputProcessorRef,
    pub root_state_access: GlobalStateAccessOutputProcessorRef,

    pub local_cache: GlobalStateOutputProcessorRef,
    pub local_cache_access: GlobalStateAccessOutputProcessorRef,
}

#[derive(Clone)]
pub(crate) struct ObjectServices {
    pub ndn_service: Arc<NDNService>,
    pub non_service: Arc<NONService>,

    pub crypto_service: Arc<CryptoService>,
    pub util_service: Arc<UtilService>,
    pub trans_service: Arc<TransService>,

    pub front_service: Option<Arc<FrontService>>,
}

pub struct CyfsStackImpl {
    config: StackGlobalConfig,

    admin_manager: AdminManager,

    bdt_stack: StackGuard,

    device_manager: DeviceInfoManager,

    zone_manager: ZoneManager,

    noc: ObjectCacheManager,

    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,

    services: ObjectServices,
    processors: CyfsStackProcessors,

    interface: OnceCell<ObjectListenerManagerRef>,

    app_controller: OnceCell<AppController>,

    router_handlers: RouterHandlersManager,
    router_events: RouterEventsManager,

    name_resolver: NameResolver,

    // role
    zone_role_manager: ZoneRoleManager,

    // 对象失败的后关联处理器，主要用于device对象的连接失败处理
    fail_handler: ObjectFailHandler,

    // acl
    acl_manager: AclManagerRef,

    // root_state
    root_state: GlobalStateService,

    // local_cache
    local_cache: GlobalStateLocalService,
}

impl CyfsStackImpl {
    // known_objects 外部指定的已知对象列表
    pub async fn open(
        bdt_param: BdtStackParams,
        param: CyfsStackParams,
        known_objects: Vec<KnownObject>,
    ) -> BuckyResult<Self> {
        Self::register_custom_objects_format();
        
        let stack_params = param.clone();
        let config = StackGlobalConfig::new(stack_params);

        let device = bdt_param.device.clone();
        let device_id = device.desc().device_id();
        let device_category = device.category().unwrap();

        let isolate = match &param.config.isolate {
            Some(v) => v.as_str(),
            None => "",
        };

        let noc =
            Self::init_raw_noc(&device_id, param.noc.noc_type, isolate, known_objects).await?;

        // 初始化data cache和tracker
        let ndc = Self::init_ndc(isolate)?;
        let tracker = Self::init_tracker(isolate)?;

        let task_manager = Self::init_task_manager(isolate).await?;
        let trans_store = create_trans_store(isolate).await?;
        let chunk_manager = Arc::new(ChunkManager::new());

        // 不使用rules的meta_client
        // 内部依赖带rule-noc，需要使用延迟绑定策略
        let raw_meta_cache = RawMetaCache::new(param.meta.target);

        // init object searcher for global use
        let obj_searcher = CompoundObjectSearcher::new(
            noc.clone_noc(),
            device_id.clone(),
            raw_meta_cache.clone_meta(),
        );

        // 带rules的meta client
        let rule_meta_cache = MetaCacheWithRule::new(raw_meta_cache.clone());

        // 内部依赖带rule-noc，需要使用延迟绑定策略
        let verifier = ObjectVerifier::new(
            bdt_param.device.desc().device_id().to_owned(),
            raw_meta_cache.clone_meta(),
        );
        let verifier = Arc::new(verifier);
        verifier.bind_noc(noc.clone_noc());

        // device_manager和zone_manager使用raw_noc
        let device_manager = DeviceInfoManager::new(
            noc.clone_noc(),
            verifier.clone(),
            obj_searcher.clone().into_ref(),
            bdt_param.device.clone(),
        );

        let fail_handler =
            ObjectFailHandler::new(raw_meta_cache.clone_meta(), device_manager.clone_cache());

        let zone_manager = ZoneManager::new(
            noc.clone_noc(),
            device_manager.clone_cache(),
            device_id.clone(),
            device_category,
            raw_meta_cache.clone_meta(),
            fail_handler.clone(),
        );
        zone_manager.init().await?;

        // handlers
        let router_handlers = RouterHandlersManager::new(param.config.isolate.clone());
        if let Err(e) = router_handlers.load().await {
            error!("load router handlers error! {}", e);
        }

        // events
        let router_events = RouterEventsManager::new();

        // acl
        let acl_manager = Arc::new(AclManager::new(
            noc.clone_noc(),
            param.config.isolate.clone(),
            device_manager.clone_cache(),
            zone_manager.clone(),
            router_handlers.clone(),
        ));

        // role manager
        let zone_role_manager = ZoneRoleManager::new(
            device_id.clone(),
            zone_manager.clone(),
            noc.clone_noc(),
            raw_meta_cache.clone(),
            acl_manager.clone(),
            router_events.clone(),
            config.clone(),
        );

        // 初始化bdt协议栈
        let bdt_stack = Self::init_bdt_stack(
            acl_manager.clone(),
            bdt_param,
            device_manager.clone_cache(),
            isolate,
            ndc.clone(),
            tracker.clone(),
            router_handlers.clone(),
            Box::new(ChunkStoreReader::new(
                chunk_manager.clone(),
                ndc.clone(),
                tracker.clone(),
            )),
        )
        .await?;

        // enable the zone search ablity for obj_searcher
        obj_searcher.init_zone_searcher(zone_manager.clone(), noc.clone_noc(), bdt_stack.clone());

        // non和router通用的转发器，不带权限检查(non和router内部根据层级选择正确的acl适配器)
        let forward_manager =
            ForwardProcessorManager::new(bdt_stack.clone(), device_manager.clone_cache());
        forward_manager.start();

        // ood_resolver
        let ood_resoler = OodResolver::new(device_id.clone(), obj_searcher.clone().into_ref());

        // crypto
        let crypto = ObjectCrypto::new(
            verifier,
            zone_manager.clone(),
            device_manager.clone_cache(),
            bdt_stack.clone(),
        );

        let crypto_service = CryptoService::new(
            crypto,
            acl_manager.clone(),
            zone_manager.clone(),
            forward_manager.clone(),
            fail_handler.clone(),
            router_handlers.clone(),
        );

        // 名字解析服务
        let name_resolver = NameResolver::new(rule_meta_cache.clone_meta(), noc.clone_noc());
        name_resolver.start().await?;

        let util_service = UtilService::new(
            acl_manager.clone(),
            noc.clone_noc(),
            ndc.clone(),
            bdt_stack.clone(),
            forward_manager.clone(),
            zone_manager.clone(),
            fail_handler.clone(),
            ood_resoler.clone(),
            task_manager.clone(),
            config.clone(),
        );

        let (non_service, ndn_service) = NONService::new(
            noc.clone_noc(),
            bdt_stack.clone(),
            ndc.clone(),
            tracker.clone(),
            forward_manager.clone(),
            acl_manager.clone(),
            zone_manager.clone(),
            ood_resoler.clone(),
            router_handlers.clone(),
            rule_meta_cache.clone_meta(),
            fail_handler.clone(),
            chunk_manager.clone(),
        );

        raw_meta_cache.bind_noc(non_service.raw_noc_processor().clone());

        let trans_service = TransService::new(
            noc.clone_noc(),
            bdt_stack.clone(),
            ndc.clone(),
            tracker.clone(),
            ood_resoler.clone(),
            chunk_manager.clone(),
            task_manager.clone(),
            acl_manager.clone(),
            forward_manager.clone(),
            zone_manager.clone(),
            fail_handler.clone(),
            trans_store,
        );

        let non_service = Arc::new(non_service);
        let ndn_service = Arc::new(ndn_service);
        let crypto_service = Arc::new(crypto_service);
        let util_service = Arc::new(util_service);

        // 加载全局状态
        let (root_state, local_cache) = Self::load_global_state(
            &device_id,
            &device,
            noc.clone_noc(),
            acl_manager.clone(),
            forward_manager.clone(),
            zone_manager.clone(),
            fail_handler.clone(),
            &non_service,
            &ndn_service,
            &config,
        )
        .await?;
        let current_root = root_state.local_service().state().get_current_root();

        let front_service = if param.front.enable {
            let app_service =
                AppService::new(&zone_manager, root_state.clone_global_state_processor()).await?;

            let front_service = FrontService::new(
                non_service.clone_processor(),
                ndn_service.clone_processor(),
                root_state.clone_access_processor(),
                local_cache.clone_access_processor(),
                app_service,
                ood_resoler.clone(),
            );
            Some(Arc::new(front_service))
        } else {
            None
        };

        let services = ObjectServices {
            ndn_service,
            non_service,

            crypto_service,
            util_service,

            trans_service: Arc::new(trans_service),

            front_service,
        };

        // 缓存所有processors，用以uni_stack直接返回使用
        let processors = CyfsStackProcessors {
            non_service: NONOutputTransformer::new(
                services.non_service.clone_processor(),
                device_id.clone(),
            ),
            ndn_service: NDNOutputTransformer::new(
                services.ndn_service.clone_processor(),
                device_id.clone(),
            ),
            crypto_service: CryptoOutputTransformer::new(
                services.crypto_service.clone_processor(),
                device_id.clone(),
            ),
            util_service: UtilOutputTransformer::new(
                services.util_service.clone_processor(),
                device_id.clone(),
            ),
            trans_service: TransOutputTransformer::new(
                services.trans_service.clone_processor(),
                device_id.clone(),
                None,
            ),
            router_handlers: router_handlers.clone_processor(),
            router_events: router_events.clone_processor(),

            root_state: GlobalStateOutputTransformer::new(
                root_state.clone_global_state_processor(),
                device_id.clone(),
            ),

            root_state_access: GlobalStateAccessOutputTransformer::new(
                root_state.clone_access_processor(),
                device_id.clone(),
            ),

            local_cache: GlobalStateOutputTransformer::new(
                local_cache.clone_global_state_processor(),
                device_id.clone(),
            ),

            local_cache_access: GlobalStateAccessOutputTransformer::new(
                local_cache.clone_access_processor(),
                device_id.clone(),
            ),
        };

        let admin_manager = AdminManager::new(
            zone_role_manager.clone(),
            services.crypto_service.local_service().verifier().clone(),
            config.clone(),
        );

        let mut stack = Self {
            config,

            admin_manager,

            bdt_stack,

            root_state,
            local_cache,

            device_manager,
            zone_manager,

            noc,

            ndc,
            tracker,

            services,
            processors,

            interface: OnceCell::new(),
            app_controller: OnceCell::new(),

            router_handlers,
            router_events,

            name_resolver,

            zone_role_manager,

            fail_handler,

            acl_manager,
        };

        // first init current zone info
        let zone_manager = stack.zone_manager.clone();
        async_std::task::spawn(async move { zone_manager.get_current_info().await }).await?;

        // root_state should never change during stack init and before role_manager init access_mode
        let now_root = stack.root_state.local_service().state().get_current_root();
        if now_root != current_root {
            error!(
                "root_state changed during stack init! {:?} => {:?}",
                current_root, now_root
            );
            unreachable!();
        }

        // init root_state access mode
        stack
            .zone_role_manager
            .init_root_state_access_mode()
            .await?;

        // 首先初始化acl
        stack.acl_manager.init().await;

        Self::init_chunk_manager(&chunk_manager, isolate).await?;

        if param.config.sync_service {
            // 避免调用栈过深，使用task异步初始化

            let (ret, s) = async_std::task::spawn(async move {
                let ret = stack
                    .zone_role_manager
                    .init(
                        &stack.root_state.local_service(),
                        &stack.bdt_stack,
                        &stack.device_manager.clone_cache(),
                        &stack.router_handlers,
                        &stack.services.util_service,
                        chunk_manager,
                    )
                    .await;

                (ret, stack)
            })
            .await;

            if let Err(e) = ret {
                error!("init role manager failed! {}", e);
                return Err(e);
            }

            stack = s;
        }

        // init admin manager
        stack.admin_manager.init(&stack.router_handlers).await?;

        // 初始化对外interface
        let mut interface = ObjectListenerManager::new(device_id.clone());
        let mut init_params = ObjectListenerManagerParams {
            bdt_stack: stack.bdt_stack.clone(),
            bdt_listeners: param.interface.bdt_listeners,
            tcp_listeners: Vec::new(),
            ws_listener: None,
        };

        // 如果开启了本地的sharestack，那么就需要初始化tcp-http接口
        if param.config.shared_stack {
            init_params.tcp_listeners = param.interface.tcp_listeners;
            init_params.ws_listener = param.interface.ws_listener;
        }

        interface.init(
            init_params,
            &stack.services,
            &stack.router_handlers,
            &stack.router_events,
            &stack.name_resolver,
            &stack.acl_manager,
            &stack.zone_role_manager,
            &stack.root_state,
            &stack.local_cache,
        );

        let interface = Arc::new(interface);
        if let Err(_) = stack.interface.set(interface.clone()) {
            unreachable!();
        }

        // init app controller
        let app_controller = AppController::new(param.config.isolate.clone(), interface);
        app_controller
            .init(&stack.router_handlers.clone_processor())
            .await?;

        if let Err(_) = stack.app_controller.set(app_controller) {
            unreachable!();
        }

        // finally start interface
        stack.interface.get().unwrap().start().await?;

        // 初始化dsg
        // stack.init_dsg(param.dsg_options).await?;

        // start rust's task thread pool and process dead lock checking
        cyfs_debug::ProcessDeadHelper::instance().start_check();

        task_manager.resume_task().await?;

        Ok(stack)
    }

    /*
    async fn init_dsg(&mut self, opt: Option<DSGServiceOptions>) -> BuckyResult<()> {
        let zone_manager = self.zone_manager.clone();
        let current_zone_info =
            async_std::task::spawn(async move { zone_manager.get_current_info().await }).await?;

        // 只有ood才开启DSG服务
        if current_zone_info.is_ood_device {
            info!("will init dsg serivce: {:?}", opt);

            // FIXME暂时使用sharedobjectstack，以后切换到依赖ObjectStack
            let stack = self.open_shared_object_stack().await?;

            // 这里一定会成功
            stack.wait_online(None).await.unwrap();

            let opt = opt.unwrap_or_else(|| DSGServiceOptions::default());

            let dsg_service = DSGService::open(stack, opt).await?;
            assert!(self.dsg_service.is_none());
            self.dsg_service = Some(dsg_service);
        }

        Ok(())
    }
    */

    async fn load_global_state(
        device_id: &DeviceId,
        device: &Device,
        noc: Box<dyn NamedObjectCache>,
        acl: AclManagerRef,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManager,
        fail_handler: ObjectFailHandler,
        non_service: &Arc<NONService>,
        ndn_service: &Arc<NDNService>,
        config: &StackGlobalConfig,
    ) -> BuckyResult<(GlobalStateService, GlobalStateLocalService)> {
        let owner = match device.desc().owner() {
            Some(owner) => owner.to_owned(),
            None => {
                warn!(
                    "current device has no owner, now will use self as owner! device={}",
                    device_id
                );
                device_id.object_id().to_owned()
            }
        };

        let ndn_processor = ndn_service.get_api(&NDNAPILevel::Router).clone();

        let noc_processor = non_service.raw_noc_processor().clone();

        // root_state
        let root_state = GlobalStateService::load(
            GlobalStateCategory::RootState,
            acl,
            &device_id,
            Some(owner),
            noc.clone_noc(),
            forward,
            zone_manager,
            fail_handler,
            noc_processor,
            ndn_processor,
            config.clone(),
        )
        .await?;
        info!(
            "load root state success! device={}, owner={}",
            device_id, owner
        );

        // local_state
        let local_state = GlobalStateLocalService::load(
            GlobalStateCategory::LocalCache,
            &device_id,
            Some(owner),
            noc,
            config.clone(),
        )
        .await?;
        info!(
            "load local cache success! device={}, owner={}",
            device_id, owner
        );

        // local state always writable
        config.change_access_mode(
            GlobalStateCategory::LocalCache,
            GlobalStateAccessMode::Write,
        );

        Ok((root_state, local_state))
    }

    async fn init_bdt_stack(
        acl: AclManagerRef,
        params: BdtStackParams,
        device_cache: Box<dyn DeviceCache>,
        isolate: &str,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        router_handlers: RouterHandlersManager,
        chunk_store: Box<dyn ChunkReader>,
    ) -> BuckyResult<StackGuard> {
        let mut bdt_params = StackOpenParams::new(isolate);

        if !params.tcp_port_mapping.is_empty() {
            bdt_params.tcp_port_mapping = Some(params.tcp_port_mapping);
        }

        if let Some(sn_only) = params.udp_sn_only {
            bdt_params.config.interface.udp.sn_only = sn_only;
        }

        if !params.known_sn.is_empty() {
            bdt_params.known_sn = Some(params.known_sn);
        }
        if !params.known_device.is_empty() {
            bdt_params.known_device = Some(params.known_device);
        }
        if !params.known_passive_pn.is_empty() {
            bdt_params.passive_pn = Some(params.known_passive_pn);
        }
        bdt_params.ndc = Some(ndc);
        bdt_params.tracker = Some(tracker);
        bdt_params.outer_cache = Some(device_cache);
        bdt_params.chunk_store = Some(chunk_store);

        bdt_params.ndn_event = Some(Box::new(BdtNdnEventHandler::new(acl, router_handlers)));

        let ret = Stack::open(params.device, params.secret, bdt_params).await;

        if let Err(e) = ret {
            error!("init bdt stack error: {}", e);
            return Err(e);
        }

        let bdt_stack = ret.unwrap();

        // 等待sn上线
        info!(
            "now will wait for sn online {}......",
            bdt_stack.local_device_id()
        );
        let begin = std::time::Instant::now();
        let net_listener = bdt_stack.net_manager().listener().clone();
        let ret = net_listener.wait_online().await;
        let during = std::time::Instant::now() - begin;
        if let Err(e) = ret {
            error!(
                "bdt stack wait sn online failed! {}, during={}s, {}",
                bdt_stack.local_device_id(),
                during.as_secs(),
                e
            );
        } else {
            info!(
                "bdt stack sn online success! {}, during={}s",
                bdt_stack.local_device_id(),
                during.as_secs()
            );
        }

        Ok(bdt_stack)
    }

    async fn init_raw_noc(
        device_id: &DeviceId,
        noc_type: NamedObjectStorageType,
        isolate: &str,
        known_objects: Vec<KnownObject>,
    ) -> BuckyResult<ObjectCacheManager> {
        let mut noc = ObjectCacheManager::new(device_id);

        let isolate = isolate.to_owned();

        // 这里切换线程同步初始化，否则debug下可能会导致主线程调用栈过深
        let noc = async_std::task::spawn(async move {
            match noc.init(noc_type, &isolate).await {
                Ok(_) => {
                    info!("init object cache manager success!");
                    Ok(noc)
                }
                Err(e) => {
                    error!("init object cache manager failed: {}", e);
                    Err(e)
                }
            }
        })
        .await?;

        // 这里异步的初始化一些已知对象
        let noc2 = noc.clone();
        let device_id = device_id.clone();
        async_std::task::spawn(async move {
            // 初始化known_objects
            for item in known_objects.into_iter() {
                let req = NamedObjectCacheInsertObjectRequest {
                    protocol: NONProtocol::Native,
                    source: device_id.clone(),
                    object_id: item.object_id,
                    dec_id: None,
                    object_raw: item.object_raw,
                    object: item.object,
                    flags: 0u32,
                };
                let _ = noc2.insert_object_with_event(&req, None).await;
            }
        });
        Ok(noc)
    }

    fn init_ndc(isolate: &str) -> BuckyResult<Box<dyn NamedDataCache>> {
        use cyfs_ndc::DataCacheManager;

        DataCacheManager::create_data_cache(isolate)
    }

    fn init_tracker(isolate: &str) -> BuckyResult<Box<dyn TrackerCache>> {
        use cyfs_tracker_cache::TrackerCacheManager;

        TrackerCacheManager::create_tracker_cache(isolate)
    }

    async fn create_task_manager(isolate: &str) -> BuckyResult<Arc<TaskManager>> {
        let mut base_dir = cyfs_util::get_cyfs_root_path();
        base_dir.push("data");
        if isolate.len() > 0 {
            base_dir.push(isolate)
        }
        base_dir.push("task-manager");

        if !base_dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&base_dir) {
                log::error!(
                    "create bdt storage dir failed! dir={}, err={}",
                    base_dir.display(),
                    e
                );
            } else {
                log::info!(
                    "create named-data-cache dir success! {}",
                    base_dir.display()
                );
            }
        }

        let task_store_path = base_dir.join("data.db");

        let task_store = Arc::new(SQLiteTaskStore::new(task_store_path).await?);
        task_store.init().await?;
        let task_manager = TaskManager::new(task_store.clone(), task_store).await?;
        Ok(task_manager)
    }

    async fn init_task_manager(isolate: &str) -> BuckyResult<Arc<TaskManager>> {
        match Self::create_task_manager(isolate).await {
            Ok(task_manager) => {
                log::info!("create task manager success!");
                Ok(task_manager)
            }
            Err(e) => {
                log::info!("create task manager failed!.{}", &e);
                Err(e)
            }
        }
    }

    async fn init_chunk_manager(
        chunk_manager: &Arc<ChunkManager>,
        isolate: &str,
    ) -> BuckyResult<()> {
        match chunk_manager.init(isolate).await {
            Ok(()) => {
                log::info!("init chunk manager success!");
                Ok(())
            }
            Err(e) => {
                log::info!("init chunk manager failed!.{}", &e);
                Err(e)
            }
        }
    }

    // 网络抖动、切换后，重置网络
    pub async fn reset_network(&self, endpoints: &Vec<Endpoint>) -> BuckyResult<()> {
        info!("will reset bdt stack endpoints: {:?}", endpoints);

        if let Err(e) = self.bdt_stack.reset(&endpoints).await {
            error!("reset bdt stack error: {}", e);
            return Err(e);
        }

        if let Some(client) = &self.zone_role_manager.sync_client() {
            client.wakeup_ping();
        }

        Ok(())
    }

    pub async fn restart_interface(&self) -> BuckyResult<()> {
        if let Some(interface) = self.interface.get() {
            interface.restart().await?;
        }

        Ok(())
    }

    pub fn prepare_shared_object_stack_param(
        &self,
        dec_id: Option<ObjectId>,
    ) -> SharedCyfsStackParam {
        let non_http_addr = self
            .interface
            .get()
            .unwrap()
            .get_available_http_listener()
            .unwrap();
        let non_http_service_url = format!("http://{}", non_http_addr);

        // 必须同时开启ws服务，用以基于ws的事件系统和http服务
        let ws_addr = self
            .interface
            .get()
            .unwrap()
            .get_ws_event_listener()
            .unwrap();
        let ws_url = format!("ws://{}", ws_addr);

        SharedCyfsStackParam::new_with_ws_event(dec_id, &non_http_service_url, &ws_url).unwrap()
    }

    pub async fn open_shared_object_stack(
        &self,
        dec_id: Option<ObjectId>,
    ) -> BuckyResult<SharedCyfsStack> {
        let param = self.prepare_shared_object_stack_param(dec_id);
        // param.requestor_config = CyfsStackRequestorConfig::ws();

        match SharedCyfsStack::open(param).await {
            Ok(stack) => Ok(stack),
            Err(e) => {
                error!("open shared object stack failed! err={}", e);
                Err(e)
            }
        }
    }

    fn register_custom_objects_format() {
        use std::sync::atomic::{AtomicBool, Ordering};
    
        static INIT_DONE: AtomicBool = AtomicBool::new(false);
        if !INIT_DONE.swap(true, Ordering::SeqCst) {
            cyfs_core::register_core_objects_format();
            cyfs_lib::register_core_objects_format();
        }
    }
    
}

#[derive(Clone)]
pub struct CyfsStack {
    stack: Arc<CyfsStackImpl>,

    // uni_stack
    uni_stack: Arc<OnceCell<UniObjectStackRef>>,
}

impl CyfsStack {
    pub async fn open(
        bdt_param: BdtStackParams,
        param: CyfsStackParams,
        known_objects: Vec<KnownObject>,
    ) -> BuckyResult<CyfsStack> {
        info!("will init object stack: {:?}", param);

        let stack_impl = CyfsStackImpl::open(bdt_param, param, known_objects).await?;
        Ok(Self {
            stack: Arc::new(stack_impl),
            uni_stack: Arc::new(OnceCell::new()),
        })
    }

    pub fn local_device_id(&self) -> &DeviceId {
        &self.stack.bdt_stack.local_device_id()
    }

    pub fn local_device(&self) -> Device {
        self.stack.bdt_stack.local()
    }

    pub fn acl_manager(&self) -> &AclManager {
        &self.stack.acl_manager
    }

    pub fn interface(&self) -> Option<&ObjectListenerManagerRef> {
        self.stack.interface.get()
    }

    pub fn bdt_stack(&self) -> &StackGuard {
        &self.stack.bdt_stack
    }

    pub fn noc_manager(&self) -> &ObjectCacheManager {
        &self.stack.noc
    }

    pub fn device_manager(&self) -> &DeviceInfoManager {
        &self.stack.device_manager
    }

    pub fn zone_manager(&self) -> &ZoneManager {
        &self.stack.zone_manager
    }

    pub fn zone_role_manager(&self) -> &ZoneRoleManager {
        &self.stack.zone_role_manager
    }

    pub fn non_service(&self) -> &Arc<NONService> {
        &self.stack.services.non_service
    }

    pub fn ndn_service(&self) -> &Arc<NDNService> {
        &self.stack.services.ndn_service
    }

    pub fn crypto_service(&self) -> &Arc<CryptoService> {
        &self.stack.services.crypto_service
    }

    pub fn router_handlers(&self) -> &RouterHandlersManager {
        &self.stack.router_handlers
    }

    pub fn router_events(&self) -> &RouterEventsManager {
        &self.stack.router_events
    }

    pub fn trans_service(&self) -> &Arc<TransService> {
        &self.stack.services.trans_service
    }

    pub fn util_service(&self) -> &Arc<UtilService> {
        &self.stack.services.util_service
    }

    pub fn root_state(&self) -> &GlobalStateService {
        &self.stack.root_state
    }

    pub fn root_state_stub(
        &self,
        target: Option<ObjectId>,
        dec_id: Option<ObjectId>,
    ) -> GlobalStateStub {
        let processor = GlobalStateOutputTransformer::new(
            self.stack.root_state.clone_global_state_processor(),
            self.zone_manager().get_current_device_id().clone(),
        );

        GlobalStateStub::new(processor, target, dec_id)
    }

    pub fn local_cache(&self) -> &GlobalStateLocalService {
        &self.stack.local_cache
    }

    pub fn local_cache_stub(&self, dec_id: Option<ObjectId>) -> GlobalStateStub {
        let processor = GlobalStateOutputTransformer::new(
            self.stack.local_cache.clone_global_state_processor(),
            self.zone_manager().get_current_device_id().clone(),
        );

        GlobalStateStub::new(processor, None, dec_id)
    }

    // 只有ood才会开启DSG服务
    //pub fn dsg_service(&self) -> Option<&DSGService> {
    //    self.stack.dsg_service.as_ref()
    //}

    pub async fn reset_network(&self, endpoints: &Vec<Endpoint>) -> BuckyResult<()> {
        self.stack.reset_network(endpoints).await
    }

    pub async fn restart_interface(&self) -> BuckyResult<()> {
        self.stack.restart_interface().await
    }

    pub async fn open_shared_object_stack(
        &self,
        dec_id: Option<ObjectId>,
    ) -> BuckyResult<SharedCyfsStack> {
        self.stack.open_shared_object_stack(dec_id).await
    }

    fn create_uni_stack(&self) -> UniObjectStackRef {
        Arc::new(self.clone())
    }

    pub fn uni_stack(&self) -> &UniObjectStackRef {
        self.uni_stack.get_or_init(|| self.create_uni_stack())
    }
}

impl UniCyfsStack for CyfsStack {
    fn non_service(&self) -> &NONOutputProcessorRef {
        &self.stack.processors.non_service
    }

    fn ndn_service(&self) -> &NDNOutputProcessorRef {
        &self.stack.processors.ndn_service
    }

    fn crypto_service(&self) -> &CryptoOutputProcessorRef {
        &self.stack.processors.crypto_service
    }

    fn util_service(&self) -> &UtilOutputProcessorRef {
        &self.stack.processors.util_service
    }

    fn trans_service(&self) -> &TransOutputProcessorRef {
        &self.stack.processors.trans_service
    }

    fn router_handlers(&self) -> &RouterHandlerManagerProcessorRef {
        &self.stack.processors.router_handlers
    }

    fn router_events(&self) -> &RouterEventManagerProcessorRef {
        &self.stack.processors.router_events
    }

    fn root_state(&self) -> &GlobalStateOutputProcessorRef {
        &self.stack.processors.root_state
    }

    fn root_state_access(&self) -> &GlobalStateAccessOutputProcessorRef {
        &self.stack.processors.root_state_access
    }

    fn local_cache(&self) -> &GlobalStateOutputProcessorRef {
        &self.stack.processors.local_cache
    }

    fn local_cache_access(&self) -> &GlobalStateAccessOutputProcessorRef {
        &self.stack.processors.local_cache_access
    }
}