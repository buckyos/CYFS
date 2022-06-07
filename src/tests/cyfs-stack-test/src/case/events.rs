use cyfs_base::*;
use cyfs_lib::*;
use zone_simulator::*;
use cyfs_util::*;

struct OnTestEvent {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterEventTestEventRequest, RouterEventTestEventResult>
    for OnTestEvent
{
    async fn call(
        &self,
        param: &RouterEventTestEventRequest,
    ) -> BuckyResult<RouterEventTestEventResult> {
        info!(
            "test event: stack={}, request={}",
            self.stack, param.request
        );
        //assert!(param.response.is_none());

        let resp = RouterEventResponse {
            call_next: false,
            handled: false,
            response: None,
        };

        Ok(resp)
    }
}

pub async fn test() {
    let stack1 = TestLoader::get_shared_stack(DeviceIndex::User1OOD)
        .uni_stack()
        .clone();

    let routine = OnTestEvent {
        stack: "first-event".to_owned(),
    };

    stack1
        .router_events()
        .test_event()
        .add_event("first-event", 0, Box::new(routine))
        .await
        .unwrap();

    let routine = OnTestEvent {
        stack: "second-event".to_owned(),
    };

    stack1
        .router_events()
        .test_event()
        .add_event("second-event", -1, Box::new(routine))
        .await
        .unwrap();

    async_std::task::sleep(std::time::Duration::from_secs(20)).await;
}
