use super::super::local::*;
use super::super::router::GlobalStateMetaServiceRouter;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::rmeta::*;
use crate::zone::ZoneManager;
use crate::AclManagerRef;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateMetaService {
    root_state_meta_router: Arc<GlobalStateMetaServiceRouter>,
    root_state_meta: GlobalStatePathMetaManagerRef,
    
    local_cache_meta_router: Arc<GlobalStateMetaServiceRouter>,
    local_cache_meta: GlobalStatePathMetaManagerRef,
}

impl GlobalStateMetaService {
    pub(crate) fn new(
        isolate: &str,
        root_state: GlobalStateOutputProcessorRef,
        noc: NamedObjectCacheRef,
        acl: AclManagerRef,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManager,
        fail_handler: ObjectFailHandler,
    ) -> Self {
        // root_state
        let root_state_meta = GlobalStatePathMetaManager::new(
            isolate,
            root_state.clone(),
            GlobalStateCategory::RootState,
            noc.clone(),
        );

        let root_state_meta = Arc::new(root_state_meta);

        let root_state_meta_router = GlobalStateMetaServiceRouter::new(
            GlobalStateCategory::RootState,
            acl.clone(),
            forward.clone(),
            zone_manager.clone(),
            fail_handler.clone(),
            root_state_meta.clone_processor(),
        );
        let root_state_meta_router = Arc::new(root_state_meta_router);

        // local-cache
        let local_cache_meta = GlobalStatePathMetaManager::new(
            isolate,
            root_state.clone(),
            GlobalStateCategory::LocalCache,
            noc,
        );

        let local_cache_meta = Arc::new(local_cache_meta);

        let local_cache_meta_router = GlobalStateMetaServiceRouter::new(
            GlobalStateCategory::LocalCache,
            acl,
            forward,
            zone_manager,
            fail_handler,
            local_cache_meta.clone_processor(),
        );
        let local_cache_meta_router = Arc::new(local_cache_meta_router);

        Self {
            root_state_meta,
            root_state_meta_router,

            local_cache_meta,
            local_cache_meta_router,
        }
    }

    pub(crate) fn get_meta_manager(&self,  category: GlobalStateCategory,) -> &GlobalStatePathMetaManagerRef {
        match category {
            GlobalStateCategory::RootState => &self.root_state_meta,
            GlobalStateCategory::LocalCache => &self.local_cache_meta,
        }
    }

    pub(crate) fn clone_local_processor(
        &self,
        category: GlobalStateCategory,
    ) -> GlobalStateMetaInputProcessorRef {
        match category {
            GlobalStateCategory::RootState => self.root_state_meta.clone_processor(),
            GlobalStateCategory::LocalCache => self.local_cache_meta.clone_processor(),
        }
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
}
