use cyfs_base::*;
use cyfs_lib::*;
use zone_simulator::*;
use cyfs_util::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;

#[derive(Clone)]
struct OnTestEvent {
    stack: String,
    fired: Arc<OnceCell<bool>>,
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

        if let Err(_) = self.fired.set(true) {
            unreachable!();
        }

        let resp = RouterEventResponse {
            call_next: true,
            handled: true,
            response: None,
        };

        Ok(resp)
    }
}

pub async fn test() {
    let stack1 = TestLoader::get_shared_stack(DeviceIndex::User1OOD)
        .uni_stack()
        .clone();

    let first_routine = OnTestEvent {
        stack: "first-event".to_owned(),
        fired: Arc::new(OnceCell::new()),
    };

    stack1
        .router_events()
        .test_event()
        .add_event("first-event", 0, Box::new(first_routine.clone()))
        .await
        .unwrap();

    let second_routine = OnTestEvent {
        stack: "second-event".to_owned(),
        fired: Arc::new(OnceCell::new()),
    };

    stack1
        .router_events()
        .test_event()
        .add_event("second-event", -1, Box::new(second_routine.clone()))
        .await
        .unwrap();

    async_std::task::sleep(std::time::Duration::from_secs(3)).await;

    emit_event().await;

    assert!(first_routine.fired.get().unwrap());
    assert!(second_routine.fired.get().unwrap());
}

async fn emit_event() {
    // 触发一个测试事件
    let stack = TestLoader::get_stack(DeviceIndex::User1OOD);

    let mut emitter = stack.router_events().events().test_event().emitter();

    let param = TestEventRequest {};
    let resp = emitter.emit(param).await;
    info!("test event resp: {}", resp);
    assert_eq!(resp.handled, true);
}