use cyfs_group::GroupManager;

use crate::{
    forward::ForwardProcessorManager,
    group::GroupInputProcessorRef,
    group_api::{GroupServiceRouter, LocalGroupService},
    ZoneManagerRef,
};

#[derive(Clone)]
pub struct GroupService {
    router: GroupInputProcessorRef,
    local_service: LocalGroupService,
}

impl GroupService {
    pub(crate) fn new(
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        group_manager: GroupManager,
    ) -> Self {
        let local_service = LocalGroupService::new(group_manager);
        let router =
            GroupServiceRouter::new(forward, zone_manager, local_service.clone_processor());

        Self {
            router,
            local_service,
        }
    }

    pub(crate) fn clone_processor(&self) -> GroupInputProcessorRef {
        self.router.clone()
    }
}
