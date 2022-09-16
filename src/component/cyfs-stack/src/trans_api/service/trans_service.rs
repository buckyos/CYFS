use crate::resolver::OodResolver;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;

use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::trans::TransInputProcessorRef;
use crate::trans_api::{LocalTransService, TransServiceRouter, TransStore};
use crate::zone::ZoneManagerRef;
use crate::AclManagerRef;
use cyfs_chunk_cache::ChunkManager;
use cyfs_task_manager::TaskManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct TransService {
    router: Arc<TransServiceRouter>,
    local_service: Arc<LocalTransService>,
}

impl TransService {
    pub(crate) fn new(
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        ood_resolver: OodResolver,
        chunk_manager: Arc<ChunkManager>,
        task_manager: Arc<TaskManager>,
        acl: AclManagerRef,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        fail_handler: ObjectFailHandler,
        trans_store: Arc<TransStore>,
    ) -> Self {
        let local_service = Arc::new(LocalTransService::new(
            noc.clone(),
            bdt_stack.clone(),
            ndc.clone(),
            tracker.clone(),
            ood_resolver.clone(),
            chunk_manager.clone(),
            task_manager.clone(),
            trans_store,
        ));
        let router = Arc::new(TransServiceRouter::new(
            forward,
            zone_manager,
            fail_handler,
            local_service.clone(),
        ));

        Self {
            router,
            local_service,
        }
    }

    pub(crate) fn clone_local_processor(&self) -> TransInputProcessorRef {
        self.local_service.clone()
    }

    pub fn clone_processor(&self) -> TransInputProcessorRef {
        self.router.clone()
    }
}
