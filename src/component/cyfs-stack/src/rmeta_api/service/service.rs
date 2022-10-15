use super::super::local::*;
use super::super::path::GlobalStateAccessRequest;
use super::super::router::GlobalStateMetaServiceRouter;
use super::default::GlobalStateDefaultMetas;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::rmeta::*;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::borrow::Cow;
use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateMetaLocalService {
    root_state_meta: GlobalStatePathMetaManagerRef,

    local_cache_meta: GlobalStatePathMetaManagerRef,
}

impl GlobalStateMetaLocalService {
    pub(crate) fn new(
        isolate: &str,
        root_state: GlobalStateOutputProcessorRef,
        noc: NamedObjectCacheRef,
    ) -> Self {
        // root_state
        let root_state_meta = GlobalStatePathMetaManager::new(
            isolate,
            root_state.clone(),
            GlobalStateCategory::RootState,
            noc.clone(),
        );

        let root_state_meta = Arc::new(root_state_meta);

        // local-cache
        let local_cache_meta = GlobalStatePathMetaManager::new(
            isolate,
            root_state.clone(),
            GlobalStateCategory::LocalCache,
            noc,
        );

        let local_cache_meta = Arc::new(local_cache_meta);

        Self {
            root_state_meta,
            local_cache_meta,
        }
    }

    pub(crate) async fn init(&self) -> BuckyResult<()> {
        GlobalStateDefaultMetas::init(&self).await
    }

    pub(crate) fn get_meta_manager(
        &self,
        category: GlobalStateCategory,
    ) -> &GlobalStatePathMetaManagerRef {
        match category {
            GlobalStateCategory::RootState => &self.root_state_meta,
            GlobalStateCategory::LocalCache => &self.local_cache_meta,
        }
    }

    pub(crate) fn clone_processor(
        &self,
        category: GlobalStateCategory,
    ) -> GlobalStateMetaInputProcessorRef {
        match category {
            GlobalStateCategory::RootState => self.root_state_meta.clone_processor(),
            GlobalStateCategory::LocalCache => self.local_cache_meta.clone_processor(),
        }
    }

    pub async fn check_access(
        &self,
        source: &RequestSourceInfo,
        req_path: &RequestGlobalStatePath,
        permissions: impl Into<AccessPermissions>,
    ) -> BuckyResult<()> {
        let rmeta = self.get_meta_manager(req_path.category());

        // 如果req_path没有指定target_dec_id，那么使用source_dec_id
        let target_dec_id = req_path.dec(source);

        let ret = rmeta
            .get_option_global_state_meta(target_dec_id, true)
            .await?;
        if ret.is_none() {
            let msg = format!("global state check rmeta but target dec rmeta not found! target_dec={}, req_path={}", target_dec_id, req_path);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        let dec_rmeta = ret.unwrap();
        let permissions = permissions.into();
        let check_req = GlobalStateAccessRequest {
            dec: Cow::Borrowed(target_dec_id),
            path: req_path.req_path(),
            source: Cow::Borrowed(source),
            permissions,
        };

        if let Err(e) = dec_rmeta.check_access(check_req) {
            error!(
                "global check check rmeta but been rejected! source={}, req_path={}, permissons={}",
                source,
                req_path,
                permissions.as_str()
            );
            return Err(e);
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct GlobalStateMetaService {
    local_service: GlobalStateMetaLocalService,

    root_state_meta_router: Arc<GlobalStateMetaServiceRouter>,
    local_cache_meta_router: Arc<GlobalStateMetaServiceRouter>,
}

impl GlobalStateMetaService {
    pub(crate) fn new(
        local_service: GlobalStateMetaLocalService,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        fail_handler: ObjectFailHandler,
    ) -> Self {
        // root-state
        let root_state_meta_router = GlobalStateMetaServiceRouter::new(
            GlobalStateCategory::RootState,
            forward.clone(),
            zone_manager.clone(),
            fail_handler.clone(),
            local_service.clone_processor(GlobalStateCategory::RootState),
        );
        let root_state_meta_router = Arc::new(root_state_meta_router);

        // local-cache
        let local_cache_meta_router = GlobalStateMetaServiceRouter::new(
            GlobalStateCategory::LocalCache,
            forward,
            zone_manager,
            fail_handler,
            local_service.clone_processor(GlobalStateCategory::LocalCache),
        );
        let local_cache_meta_router = Arc::new(local_cache_meta_router);

        Self {
            local_service,

            root_state_meta_router,
            local_cache_meta_router,
        }
    }

    pub(crate) fn get_local_service(&self) -> &GlobalStateMetaLocalService {
        &self.local_service
    }

    pub(crate) fn get_meta_manager(
        &self,
        category: GlobalStateCategory,
    ) -> &GlobalStatePathMetaManagerRef {
        self.local_service.get_meta_manager(category)
    }

    pub(crate) fn clone_local_processor(
        &self,
        category: GlobalStateCategory,
    ) -> GlobalStateMetaInputProcessorRef {
        self.local_service.clone_processor(category)
    }

    pub fn clone_processor(
        &self,
        category: GlobalStateCategory,
    ) -> GlobalStateMetaInputProcessorRef {
        match category {
            GlobalStateCategory::RootState => self.root_state_meta_router.clone_processor(),
            GlobalStateCategory::LocalCache => self.local_cache_meta_router.clone_processor(),
        }
    }

    pub async fn check_access(
        &self,
        source: &RequestSourceInfo,
        req_path: &RequestGlobalStatePath,
        op_type: RequestOpType,
    ) -> BuckyResult<()> {
        self.local_service
            .check_access(source, req_path, op_type)
            .await
    }
}
