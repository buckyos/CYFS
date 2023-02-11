use crate::resolver::OodResolver;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;

use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::trans::TransInputProcessorRef;
use crate::trans_api::{LocalTransService, TransServiceRouter, TransStore};
use crate::zone::ZoneManagerRef;
use crate::{AclManagerRef, NamedDataComponents};
use cyfs_task_manager::TaskManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct TransService {
    router: TransInputProcessorRef,
    local_service: LocalTransService,
}

impl TransService {
    pub(crate) fn new(
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
        named_data_components: &NamedDataComponents,
        ood_resolver: OodResolver,
        task_manager: Arc<TaskManager>,
        _acl: AclManagerRef,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        fail_handler: ObjectFailHandler,
        trans_store: Arc<TransStore>,
    ) -> Self {
        let local_service = LocalTransService::new(
            noc.clone(),
            bdt_stack.clone(),
            named_data_components,
            ood_resolver.clone(),
            task_manager.clone(),
            trans_store,
        );
        let router = TransServiceRouter::new(
            forward,
            zone_manager,
            fail_handler,
            local_service.clone_processor(),
        );

        Self {
            router,
            local_service,
        }
    }

    pub fn clone_processor(&self) -> TransInputProcessorRef {
        self.router.clone()
    }
}
