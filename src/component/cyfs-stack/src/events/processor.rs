use super::event::RouterEvent;
use super::event_manager::*;
use cyfs_base::*;
use cyfs_util::*;
use cyfs_lib::*;

macro_rules! declare_router_event_processor {
    ($REQ:ty, $RESP:ty, $func:ident) => {
        #[async_trait::async_trait]
        impl RouterEventProcessor<$REQ, $RESP> for RouterEventsManager {
            async fn add_event(
                &self,
                id: &str,
                index: i32,
                routine: Box<
                    dyn EventListenerAsyncRoutine<
                        RouterEventRequest<$REQ>,
                        RouterEventResponse<$RESP>,
                    >,
                >,
            ) -> BuckyResult<()> {
                let event = RouterEvent::new(id.to_owned(), None, index, routine)?;

                self.events().$func().add_event(event)
            }

            async fn remove_event(&self, id: &str) -> BuckyResult<bool> {
                let ret = self.events().$func().remove_event(id, None);

                Ok(ret)
            }
        }
    };
}

// non events
declare_router_event_processor!(TestEventRequest, TestEventResponse, test_event);

impl RouterEventManagerProcessor for RouterEventsManager {
    fn test_event(&self) -> &dyn RouterEventProcessor<TestEventRequest, TestEventResponse> {
        self
    }
}
