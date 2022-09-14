use super::super::local::UtilLocalService;
use super::super::router::UtilRouter;
use crate::config::StackGlobalConfig;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::resolver::OodResolver;
use crate::util::*;
use crate::zone::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;
use cyfs_task_manager::TaskManager;
use std::sync::Arc;

pub struct UtilService {
    local_service: UtilLocalService,
    router: UtilRouter,
}

impl UtilService {
    pub(crate) fn new(
        noc: NamedObjectCacheRef,
        ndc: Box<dyn NamedDataCache>,
        bdt_stack: StackGuard,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        fail_handler: ObjectFailHandler,
        ood_resolver: OodResolver,
        task_manager: Arc<TaskManager>,
        config: StackGlobalConfig,
    ) -> Self {
        let local_service = UtilLocalService::new(
            noc,
            ndc,
            bdt_stack.clone(),
            zone_manager.clone(),
            ood_resolver,
            task_manager,
            config,
        );

        let router = UtilRouter::new(
            local_service.clone(),
            zone_manager,
            forward,
            fail_handler,
        );

        Self {
            local_service,
            router,
        }
    }

    pub(crate) fn local_service(&self) -> &UtilLocalService {
        &self.local_service
    }

    pub fn clone_local_processor(&self) -> UtilInputProcessorRef {
        self.local_service.clone_processor()
    }

    pub fn clone_processor(&self) -> UtilInputProcessorRef {
        self.router.clone_processor()
    }
}
