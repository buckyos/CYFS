// use super::dsg::{DSGService, DSGServiceOptions};
use super::params::*;
use super::uni_stack::*;
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
use crate::ndn_api::{BdtNDNEventHandler, NDNService};
use crate::non::NONOutputTransformer;
use crate::non_api::NONService;
use crate::resolver::{CompoundObjectSearcher, DeviceInfoManager, OodResolver};
use crate::rmeta::GlobalStateMetaOutputTransformer;
use crate::rmeta_api::{GlobalStateMetaLocalService, GlobalStateMetaService};
use crate::root_state::{GlobalStateAccessorOutputTransformer, GlobalStateOutputTransformer};
use crate::root_state_api::{
    GlobalStateLocalService, GlobalStateManager, GlobalStateService, GlobalStateValidatorManager,
};
use crate::router_handler::RouterHandlersManager;
use crate::trans::TransOutputTransformer;
use crate::trans_api::{create_trans_store, TransService};
use crate::util::UtilOutputTransformer;
use crate::util_api::UtilService;
use crate::zone::{ZoneManager, ZoneManagerRef, ZoneRoleManager};
use cyfs_base::*;
use cyfs_bdt::{DeviceCache, SnStatus, StackGuard};
use cyfs_bdt_ext::{BdtStackParams, NamedDataComponents};
use cyfs_lib::*;
use cyfs_noc::*;
use cyfs_task_manager::{SQLiteTaskStore, TaskManager};

use once_cell::sync::OnceCell;
use std::sync::Arc;

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

    zone_manager: ZoneManagerRef,

    noc: NamedObjectCacheRef,

    named_data_components: NamedDataComponents,

    services: ObjectServices,

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

    // global state manager
    global_state_manager: GlobalStateManager,

    // root_state
    root_state: GlobalStateService,

    // local_cache
    local_cache: GlobalStateLocalService,

    // global_state_meta
    global_state_meta: GlobalStateMetaService,
}

impl CyfsStackImpl {
    pub async fn open(
        bdt_param: BdtStackParams,
        param: CyfsStackParams,
        known_objects: CyfsStackKnownObjects, // known_objects 外部指定的已知对象列表
    ) -> BuckyResult<Self> {
        Self::register_custom_objects_format();

        let stack_params = param.clone();
        let config = StackGlobalConfig::new(stack_params, bdt_param.clone());

        let device = bdt_param.device.clone();
        let device_id = device.desc().device_id();
        let device_category = device.category().unwrap();

        let isolate = match &param.config.isolate {
            Some(v) => v.as_str(),
            None => "",
        };

        let noc = Self::init_raw_noc(isolate, known_objects).await?;
        let noc_relation = NamedObjectRelationCacheManager::create(isolate)
        .await?;

        // meta with cache
        let raw_meta_cache = RawMetaCache::new(param.meta.target, noc.clone());

        // 名字解析服务
        let name_resolver = NameResolver::new(raw_meta_cache.clone(), noc.clone());
        name_resolver.start().await?;

        // init global state manager
        let global_state_manager = GlobalStateManager::new(noc.clone(), config.clone());
        global_state_manager.load().await.map_err(|e| {
            let msg = format!("init global state manager failed! {}", e);
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        // load current zone's global_state
        let (local_root_state, local_cache) = Self::load_global_state(
            &global_state_manager,
            &device_id,
            &device,
            noc.clone(),
            &config,
        )
        .await?;

        let current_root = local_root_state.state().get_current_root();

        let task_manager = Self::init_task_manager(isolate).await?;
        let trans_store = create_trans_store(isolate).await?;
        // let chunk_manager = Arc::new(ChunkManager::new());

        // init sn config manager
        let root_state_processor = GlobalStateOutputTransformer::new(
            local_root_state.clone_global_state_processor(),
            RequestSourceInfo::new_local_system(),
        );
        let sn_config_manager = SNConfigManager::new(
            name_resolver.clone(),
            raw_meta_cache.clone(),
            root_state_processor,
            noc.clone(),
            config.clone(),
        );
        sn_config_manager.init().await?;

        // init object searcher for global use
        let obj_searcher = CompoundObjectSearcher::new(
            noc.clone(),
            device_id.clone(),
            raw_meta_cache.clone_meta(),
        );

        // init signs verifier
        let verifier = ObjectVerifier::new(
            bdt_param.device.desc().device_id().to_owned(),
            raw_meta_cache.clone_meta(),
        );
        let verifier = Arc::new(verifier);
        verifier.bind_noc(noc.clone());

        // device_manager和zone_manager使用raw_noc
        let device_manager = DeviceInfoManager::new(
            noc.clone(),
            verifier.clone(),
            obj_searcher.clone().into_ref(),
            bdt_param.device.clone(),
        );

        let named_data_components = cyfs_bdt_ext::BdtStackHelper::init_named_data_components(
            isolate,
            noc.clone(),
            device_manager.clone_cache(),
        )
        .await?;

        let fail_handler =
            ObjectFailHandler::new(raw_meta_cache.clone(), device_manager.clone_cache());

        // Init zone manager
        let root_state_processor = GlobalStateOutputTransformer::new(
            local_root_state.clone_global_state_processor(),
            RequestSourceInfo::new_local_system(),
        );
        let local_cache_processor = GlobalStateOutputTransformer::new(
            local_cache.clone_global_state_processor(),
            RequestSourceInfo::new_local_system(),
        );

        let zone_manager = ZoneManager::new(
            noc.clone(),
            device_manager.clone_cache(),
            device_id.clone(),
            device_category,
            raw_meta_cache.clone(),
            fail_handler.clone(),
            root_state_processor,
            local_cache_processor,
        );
        zone_manager.init().await?;

        // first init current zone info
        let zm = zone_manager.clone();
        async_std::task::spawn(async move { zm.get_current_info().await }).await?;

        // FIXME Which dec-id should choose to use for uni-stack's source? now use anonymous dec as default
        let source = zone_manager.get_current_source_info(&None).await?;

        // load local global-state meta
        let local_global_state_meta =
            Self::load_global_state_meta(isolate, &local_root_state, noc.clone(), &source);

        // init global-state validator
        let validator =
            GlobalStateValidatorManager::new(&device_id, &local_root_state, &local_cache);

        noc.bind_object_meta_access_provider(Arc::new(Box::new(local_global_state_meta.clone())));

        // acl
        let acl_manager = Arc::new(AclManager::new(
            local_global_state_meta.clone(),
            validator,
            noc.clone(),
            param.config.isolate.clone(),
            zone_manager.clone(),
        ));

        // handlers
        let router_handlers =
            RouterHandlersManager::new(param.config.isolate.clone(), acl_manager.clone());
        if let Err(e) = router_handlers.load().await {
            error!("load router handlers error! {}", e);
        }

        // events
        let router_events = RouterEventsManager::new();

        // role manager
        let zone_role_manager = ZoneRoleManager::new(
            device_id.clone(),
            zone_manager.clone(),
            noc.clone(),
            raw_meta_cache.clone(),
            acl_manager.clone(),
            router_events.clone(),
            config.clone(),
        );

        // 初始化bdt协议栈
        let (bdt_stack, bdt_event) = Self::init_bdt_stack(
            zone_manager.clone(),
            acl_manager.clone(),
            bdt_param,
            device_manager.clone_cache(),
            isolate,
            &named_data_components,
            router_handlers.clone(),
            &sn_config_manager,
        )
        .await?;

        named_data_components.bind_bdt_stack(bdt_stack.clone());

        // enable the zone search ablity for obj_searcher
        obj_searcher.init_zone_searcher(zone_manager.clone(), noc.clone(), bdt_stack.clone());

        // non和router通用的转发器，不带权限检查(non和router内部根据层级选择正确的acl适配器)
        let forward_manager =
            ForwardProcessorManager::new(bdt_stack.clone(), device_manager.clone_cache(), fail_handler.clone());
        forward_manager.start();

        // ood_resolver
        let ood_resoler = OodResolver::new(
            device_id.clone(),
            device_manager.clone_cache(),
            obj_searcher.clone().into_ref(),
        );

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

        let util_service = UtilService::new(
            noc.clone(),
            named_data_components.ndc.clone(),
            bdt_stack.clone(),
            forward_manager.clone(),
            zone_manager.clone(),
            fail_handler.clone(),
            ood_resoler.clone(),
            task_manager.clone(),
            config.clone(),
        );

        let (non_service, ndn_service) = NONService::new(
            noc.clone(),
            noc_relation,
            bdt_stack.clone(),
            &named_data_components,
            forward_manager.clone(),
            acl_manager.clone(),
            zone_manager.clone(),
            router_handlers.clone(),
            raw_meta_cache.clone(),
            fail_handler.clone(),
        );

        bdt_event.bind_non_processor(non_service.rmeta_noc_processor().clone());

        let trans_service = TransService::new(
            noc.clone(),
            bdt_stack.clone(),
            &named_data_components,
            ood_resoler.clone(),
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

        // load root-state service
        let root_state = Self::load_root_state_service(
            local_root_state,
            acl_manager.clone(),
            forward_manager.clone(),
            zone_manager.clone(),
            fail_handler.clone(),
            &non_service,
            &ndn_service,
        )
        .await?;

        // load global state meta service
        let global_state_meta = Self::load_global_state_meta_service(
            local_global_state_meta,
            forward_manager.clone(),
            zone_manager.clone(),
            fail_handler.clone(),
        );

        let front_service = if param.front.enable {
            let app_service =
                AppService::new(&zone_manager, root_state.clone_global_state_processor()).await?;

            let front_service = FrontService::new(
                non_service.clone_processor(),
                ndn_service.clone_processor(),
                root_state.clone_accessor_processor(),
                local_cache.clone_accessor_processor(),
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

        let admin_manager = AdminManager::new(
            zone_role_manager.clone(),
            services.crypto_service.local_service().verifier().clone(),
            config.clone(),
        );

        let mut stack = Self {
            config,

            admin_manager,

            bdt_stack,

            global_state_manager,
            root_state,
            local_cache,

            global_state_meta,

            device_manager,
            zone_manager,

            noc,

            named_data_components,

            services,

            interface: OnceCell::new(),
            app_controller: OnceCell::new(),

            router_handlers,
            router_events,

            name_resolver,

            zone_role_manager,

            fail_handler,

            acl_manager,
        };

        // init an system-dec router-handler processor for later use
        let mut system_source = source.clone();
        system_source.dec = cyfs_core::get_system_dec_app().to_owned();
        let system_router_handlers = stack.router_handlers.clone_processor(system_source);

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
        stack.acl_manager.init().await?;

        if param.config.sync_service {
            // 避免调用栈过深，使用task异步初始化

            let system_router_handlers = system_router_handlers.clone();
            let named_data_components = stack.named_data_components.clone();
            let (ret, s) = async_std::task::spawn(async move {
                let ret = stack
                    .zone_role_manager
                    .init(
                        &stack.root_state.local_service(),
                        &stack.bdt_stack,
                        &stack.device_manager.clone_cache(),
                        &system_router_handlers,
                        &stack.services.util_service,
                        named_data_components,
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
        stack.admin_manager.init(&system_router_handlers).await?;

        // bind bdt stack and start sync
        sn_config_manager.bind_bdt_stack(stack.bdt_stack.clone());

        // Init the interface for external service
        let mut interface = ObjectListenerManager::new(device_id.clone());
        let mut init_params = ObjectListenerManagerParams {
            bdt_stack: stack.bdt_stack.clone(),
            bdt_listeners: param.interface.bdt_listeners,
            tcp_listeners: Vec::new(),
            ws_listener: None,
        };

        // If the local shared_stack is turned on, then need to initialize the TCP-HTTP interface
        if param.config.shared_stack {
            init_params.tcp_listeners = param.interface.tcp_listeners;
            init_params.ws_listener = param.interface.ws_listener;
        }

        interface.init(
            init_params,
            &stack.config,
            &stack.services,
            &stack.router_handlers,
            &stack.router_events,
            &stack.name_resolver,
            &stack.acl_manager,
            &stack.zone_role_manager,
            &stack.root_state,
            &stack.local_cache,
            &stack.global_state_meta,
        );

        let interface = Arc::new(interface);
        if let Err(_) = stack.interface.set(interface.clone()) {
            unreachable!();
        }

        // init app controller
        let app_controller = AppController::new(param.config.isolate.clone(), interface);
        app_controller.init(&system_router_handlers).await?;

        if let Err(_) = stack.app_controller.set(app_controller) {
            unreachable!();
        }

        
        // finally start interface
        stack.interface.get().unwrap().start().await?;

        // start rust's task thread pool and process dead lock checking
        cyfs_debug::ProcessDeadHelper::instance().start_check();

        // try resume all tasks
        async_std::task::spawn(async move {
            if let Err(e) = task_manager.resume_task().await {
                error!("resume tasks failed! {}", e);
            }
        });

        if param.config.perf_service {
            let _ = stack.init_perf().await;
        }

        Ok(stack)
    }

    async fn load_global_state(
        global_state_manager: &GlobalStateManager,
        device_id: &DeviceId,
        device: &Device,
        noc: NamedObjectCacheRef,
        config: &StackGlobalConfig,
    ) -> BuckyResult<(GlobalStateLocalService, GlobalStateLocalService)> {
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

        // load root state
        let root_state = GlobalStateLocalService::load(
            global_state_manager,
            GlobalStateCategory::RootState,
            device_id,
            Some(owner.clone()),
            noc.clone(),
        )
        .await?;

        // make sure the system dec root state is created
        config.change_access_mode(GlobalStateCategory::RootState, GlobalStateAccessMode::Write);
        root_state
            .state()
            .get_dec_root_manager(cyfs_core::get_system_dec_app(), true)
            .await?;
        config.change_access_mode(GlobalStateCategory::RootState, GlobalStateAccessMode::Read);

        // load local cache
        let local_cache = GlobalStateLocalService::load(
            global_state_manager,
            GlobalStateCategory::LocalCache,
            device_id,
            Some(owner),
            noc,
        )
        .await?;

        // local cache always writable
        config.change_access_mode(
            GlobalStateCategory::LocalCache,
            GlobalStateAccessMode::Write,
        );

        info!(
            "load local global state success! device={}, owner={}",
            device_id, owner
        );

        Ok((root_state, local_cache))
    }

    async fn load_root_state_service(
        local_service: GlobalStateLocalService,
        acl: AclManagerRef,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        fail_handler: ObjectFailHandler,
        non_service: &Arc<NONService>,
        ndn_service: &Arc<NDNService>,
    ) -> BuckyResult<GlobalStateService> {
        let ndn_processor = ndn_service.get_api(&NDNAPILevel::Router).clone();
        let noc_processor = non_service.raw_noc_processor().clone();

        // root_state
        let root_state = GlobalStateService::load(
            GlobalStateCategory::RootState,
            local_service,
            acl,
            forward,
            zone_manager,
            fail_handler,
            noc_processor,
            ndn_processor,
        )
        .await?;
        info!("load root state service success!",);

        Ok(root_state)
    }

    fn load_global_state_meta(
        isolate: &str,
        root_state: &GlobalStateLocalService,
        noc: NamedObjectCacheRef,
        source: &RequestSourceInfo,
    ) -> GlobalStateMetaLocalService {
        let processor = root_state.clone_global_state_processor();
        let processor = GlobalStateOutputTransformer::new(processor, source.clone());

        GlobalStateMetaLocalService::new(
            isolate,
            processor,
            root_state.clone(),
            noc.clone(),
            source.zone.device.as_ref().unwrap().clone(),
        )
    }

    fn load_global_state_meta_service(
        local_service: GlobalStateMetaLocalService,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        fail_handler: ObjectFailHandler,
    ) -> GlobalStateMetaService {
        let global_state_meta =
            GlobalStateMetaService::new(local_service, forward, zone_manager, fail_handler);

        global_state_meta
    }

    async fn init_bdt_stack(
        zone_manager: ZoneManagerRef,
        acl: AclManagerRef,
        mut params: BdtStackParams,
        device_cache: Box<dyn DeviceCache>,
        isolate: &str,
        named_data_components: &NamedDataComponents,
        router_handlers: RouterHandlersManager,
        sn_config_manager: &SNConfigManager,
    ) -> BuckyResult<(StackGuard, BdtNDNEventHandler)> {
        let event =
            BdtNDNEventHandler::new(zone_manager, acl, router_handlers, named_data_components);

        // priority: params sn(always loaded from config dir) > sn config manager(always loaded from meta) > buildin sn
        if params.known_sn.is_empty() {
            // use sn from sn config manager
            let mut sn_list = sn_config_manager.get_sn_list();
            if sn_list.is_empty() {
                sn_list = cyfs_util::get_builtin_sn_desc().clone();
            }

            params.known_sn = sn_list.into_iter().map(|v| v.1).collect();
        }

        let bdt_stack = cyfs_bdt_ext::BdtStackHelper::init_bdt_stack(
            params,
            device_cache,
            isolate,
            named_data_components,
            Some(Box::new(event.clone())),
        )
        .await?;

        Ok((bdt_stack, event))
    }

    async fn init_raw_noc(
        isolate: &str,
        known_objects: CyfsStackKnownObjects,
    ) -> BuckyResult<NamedObjectCacheRef> {
        let isolate = isolate.to_owned();

        // 这里切换线程同步初始化，否则debug下可能会导致主线程调用栈过深
        let noc = async_std::task::spawn(async move {
            match NamedObjectCacheManager::create(&isolate).await {
                Ok(noc) => {
                    info!("init named object cache manager success!");
                    Ok(noc)
                }
                Err(e) => {
                    error!("init named object cache manager failed: {}", e);
                    Err(e)
                }
            }
        })
        .await?;

        // 这里异步的初始化一些已知对象
        let noc2 = noc.clone();
        let task = async_std::task::spawn(async move {
            // 初始化known_objects
            for object in known_objects.list.into_iter() {
                let req = NamedObjectCachePutObjectRequest {
                    source: RequestSourceInfo::new_local_system(),
                    object,
                    storage_category: NamedObjectStorageCategory::Storage,
                    context: None,
                    last_access_rpath: None,
                    access_string: None,
                };
                let _ = noc2.put_object(&req).await;
            }
        });

        if known_objects.mode == CyfsStackKnownObjectsInitMode::Sync {
            task.await;
        }

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

    // 网络抖动、切换后，重置网络
    pub async fn reset_network(&self, endpoints: &Vec<Endpoint>) -> BuckyResult<()> {
        info!("will reset bdt stack endpoints: {:?}", endpoints);

        match self
            .bdt_stack
            .reset_endpoints(&endpoints)
            .await
            .wait_online()
            .await
        {
            Err(err) => {
                error!("reset bdt stack error: {}", err);
                return Err(err);
            }
            Ok(status) => {
                if status == SnStatus::Offline {
                    error!("reset bdt stack error: offline");
                    return Err(BuckyError::new(BuckyErrorCode::Failed, "offline"));
                }
            }
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
        requestor_config: Option<CyfsStackRequestorConfig>,
    ) -> BuckyResult<SharedCyfsStackParam> {
        let non_http_addr = self
            .interface
            .get()
            .unwrap()
            .get_available_http_listener()
            .ok_or_else(|| {
                let msg = format!("http interface not valid!");
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::NotSupport, msg)
            })?;

        let non_http_service_url = format!("http://{}", non_http_addr);

        // 必须同时开启ws服务，用以基于ws的事件系统和http服务
        let ws_addr = self
            .interface
            .get()
            .unwrap()
            .get_ws_event_listener()
            .ok_or_else(|| {
                let msg = format!("ws interface not valid!");
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::NotSupport, msg)
            })?;

        let ws_url = format!("ws://{}", ws_addr);

        let mut param =
            SharedCyfsStackParam::new_with_ws_event(dec_id, &non_http_service_url, &ws_url)
                .unwrap();
        if let Some(requestor_config) = requestor_config {
            param.requestor_config = requestor_config;
        }

        Ok(param)
    }

    pub async fn open_shared_object_stack(
        &self,
        dec_id: Option<ObjectId>,
        requestor_config: Option<CyfsStackRequestorConfig>,
    ) -> BuckyResult<SharedCyfsStack> {
        let param = self.prepare_shared_object_stack_param(dec_id, requestor_config)?;

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

    pub async fn open_uni_stack(&self, dec_id: &Option<ObjectId>) -> UniCyfsStackRef {
        let processors = self.gen_processors(dec_id).await;
        Arc::new(processors)
    }

    async fn gen_processors(&self, dec_id: &Option<ObjectId>) -> CyfsStackProcessors {
        let source = self
            .zone_manager
            .get_current_source_info(dec_id)
            .await
            .unwrap();

        // 缓存所有processors，用以uni_stack直接返回使用
        let processors = CyfsStackProcessors {
            non_service: NONOutputTransformer::new(
                self.services.non_service.clone_processor(),
                source.clone(),
            ),
            ndn_service: NDNOutputTransformer::new(
                self.services.ndn_service.clone_processor(),
                source.clone(),
            ),
            crypto_service: CryptoOutputTransformer::new(
                self.services.crypto_service.clone_processor(),
                source.clone(),
            ),
            util_service: UtilOutputTransformer::new(
                self.services.util_service.clone_processor(),
                source.clone(),
            ),
            trans_service: TransOutputTransformer::new(
                self.services.trans_service.clone_processor(),
                source.clone(),
            ),
            router_handlers: self.router_handlers.clone_processor(source.clone()),
            router_events: self.router_events.clone_processor(),

            root_state: GlobalStateOutputTransformer::new(
                self.root_state.clone_global_state_processor(),
                source.clone(),
            ),

            root_state_accessor: GlobalStateAccessorOutputTransformer::new(
                self.root_state.clone_accessor_processor(),
                source.clone(),
            ),

            local_cache: GlobalStateOutputTransformer::new(
                self.local_cache.clone_global_state_processor(),
                source.clone(),
            ),

            local_cache_accessor: GlobalStateAccessorOutputTransformer::new(
                self.local_cache.clone_accessor_processor(),
                source.clone(),
            ),

            root_state_meta: GlobalStateMetaOutputTransformer::new(
                self.global_state_meta
                    .clone_processor(GlobalStateCategory::RootState),
                source.clone(),
            ),
            local_cache_meta: GlobalStateMetaOutputTransformer::new(
                self.global_state_meta
                    .clone_processor(GlobalStateCategory::LocalCache),
                source.clone(),
            ),
        };

        processors
    }

    async fn init_perf(&self) -> BuckyResult<()> {
        use cyfs_perf_client::*;

        // The same process can only be initialized once, there may be other cyfs-stacks in the same process
        if !cyfs_base::PERF_MANGER.get().is_none() {
            warn!("perf manager already initialized!");
            return Ok(());
        } 

        let info = self.zone_manager.get_current_info().await?;

        let perf = PerfClient::new(
            "cyfs-stack".to_owned(),
            cyfs_base::get_version().to_owned(),
            None,
            PerfConfig {
                reporter: PerfServerConfig::Default,
                save_to_file: true,
                report_interval: std::time::Duration::from_secs(60 * 10),
            },
            self.open_uni_stack(&None).await,
            info.device_id.clone(),
            info.owner_id.clone(),
        );

        if let Err(e) = perf.start().await {
            error!("init perf client failed! {}", e);
        }

        if let Err(_) = cyfs_base::PERF_MANGER.set(Box::new(perf)) {
            warn!("init perf manager but already initialized!");
        }

        info!("init perf manager success! current={}", info.device_id);

        Ok(())
    }
}

#[derive(Clone)]
pub struct CyfsStack {
    stack: Arc<CyfsStackImpl>,
}

impl CyfsStack {
    pub async fn open(
        bdt_param: BdtStackParams,
        param: CyfsStackParams,
        known_objects: CyfsStackKnownObjects,
    ) -> BuckyResult<CyfsStack> {
        info!("will init object stack: {:?}", param);

        let stack_impl = CyfsStackImpl::open(bdt_param, param, known_objects).await?;
        Ok(Self {
            stack: Arc::new(stack_impl),
        })
    }

    pub fn local_device_id(&self) -> &DeviceId {
        &self.stack.bdt_stack.local_device_id()
    }

    pub fn local_device(&self) -> Device {
        self.stack.bdt_stack.sn_client().ping().default_local()
    }

    pub fn config(&self) -> &StackGlobalConfig {
        &self.stack.config
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

    pub fn noc_manager(&self) -> &NamedObjectCacheRef {
        &self.stack.noc
    }

    pub fn device_manager(&self) -> &DeviceInfoManager {
        &self.stack.device_manager
    }

    pub fn zone_manager(&self) -> &ZoneManagerRef {
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

    pub fn global_state_manager(&self) -> &GlobalStateManager {
        &self.stack.global_state_manager
    }

    pub fn root_state(&self) -> &GlobalStateService {
        &self.stack.root_state
    }

    // use system dec as default dec
    pub async fn root_state_stub(
        &self,
        target: Option<ObjectId>,
        target_dec_id: Option<ObjectId>,
    ) -> GlobalStateStub {
        let source = self
            .zone_manager()
            .get_current_source_info(&Some(cyfs_core::get_system_dec_app().to_owned()))
            .await
            .unwrap();
        let processor = GlobalStateOutputTransformer::new(
            self.stack.root_state.clone_global_state_processor(),
            source,
        );

        GlobalStateStub::new(processor, target, target_dec_id)
    }

    pub fn local_cache(&self) -> &GlobalStateLocalService {
        &self.stack.local_cache
    }

    // use system dec as default dec
    pub async fn local_cache_stub(&self, target_dec_id: Option<ObjectId>) -> GlobalStateStub {
        let source = self
            .zone_manager()
            .get_current_source_info(&Some(cyfs_core::get_system_dec_app().to_owned()))
            .await
            .unwrap();
        let processor = GlobalStateOutputTransformer::new(
            self.stack.local_cache.clone_global_state_processor(),
            source,
        );

        GlobalStateStub::new(processor, None, target_dec_id)
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

    pub fn prepare_shared_object_stack_param(
        &self,
        dec_id: Option<ObjectId>,
        requestor_config: Option<CyfsStackRequestorConfig>,
    ) -> BuckyResult<SharedCyfsStackParam> {
        self.stack
            .prepare_shared_object_stack_param(dec_id, requestor_config)
    }

    pub async fn open_shared_object_stack(
        &self,
        dec_id: Option<ObjectId>,
        requestor_config: Option<CyfsStackRequestorConfig>,
    ) -> BuckyResult<SharedCyfsStack> {
        self.stack
            .open_shared_object_stack(dec_id, requestor_config)
            .await
    }

    pub async fn open_uni_stack(&self, dec_id: &Option<ObjectId>) -> UniCyfsStackRef {
        self.stack.open_uni_stack(dec_id).await
    }
}
