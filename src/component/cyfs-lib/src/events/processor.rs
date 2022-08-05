use super::category::*;
use super::request::*;
use cyfs_base::*;
use cyfs_util::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait RouterEventProcessor<REQ, RESP>: Send + Sync
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + std::fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + std::fmt::Display,
    RouterEventRequest<REQ>: RouterEventCategoryInfo,
{
    async fn add_event(
        &self,
        id: &str,
        index: i32,
        routine: Box<
            dyn EventListenerAsyncRoutine<RouterEventRequest<REQ>, RouterEventResponse<RESP>>,
        >,
    ) -> BuckyResult<()>;

    async fn remove_event(&self, id: &str) -> BuckyResult<bool>;
}

pub trait RouterEventManagerProcessor: Send + Sync {
    fn test_event(&self) -> &dyn RouterEventProcessor<TestEventRequest, TestEventResponse>;
    fn zone_role_changed_event(&self) -> &dyn RouterEventProcessor<ZoneRoleChangedEventRequest, ZoneRoleChangedEventResponse>;
}

pub type RouterEventManagerProcessorRef = Arc<Box<dyn RouterEventManagerProcessor>>;
