use super::super::local::GlobalStateLocalService;
use super::super::router::GlobalStateRouter;
use crate::acl::AclManagerRef;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::ndn::NDNInputProcessorRef;
use crate::non::NONInputProcessorRef;
use crate::root_state::*;
use crate::zone::ZoneManager;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateService {
    local_service: GlobalStateLocalService,

    router: Arc<GlobalStateRouter>,
}

impl GlobalStateService {
    pub(crate) async fn load(
        category: GlobalStateCategory,
        local_service: GlobalStateLocalService,
        acl: AclManagerRef,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManager,
        fail_handler: ObjectFailHandler,
        noc_processor: NONInputProcessorRef,
        ndn_processor: NDNInputProcessorRef,
    ) -> BuckyResult<Self> {
        assert_eq!(category, GlobalStateCategory::RootState);

        let router = GlobalStateRouter::new(
            category,
            acl,
            local_service.clone(),
            zone_manager,
            forward,
            fail_handler,
            noc_processor,
            ndn_processor,
        );

        let ret = Self {
            local_service,
            router: Arc::new(router),
        };

        Ok(ret)
    }

    pub(crate) fn local_service(&self) -> &GlobalStateLocalService {
        &self.local_service
    }

    pub fn clone_global_state_processor(&self) -> GlobalStateInputProcessorRef {
        self.router.clone_global_state_processor()
    }

    pub fn clone_op_env_processor(&self) -> OpEnvInputProcessorRef {
        self.router.clone_op_env_processor()
    }

    pub fn clone_access_processor(&self) -> GlobalStateAccessInputProcessorRef {
        self.router.clone_access_processor()
    }
}
