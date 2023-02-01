use cyfs_lib::*;
use zone_simulator::*;

pub async fn test() {
    
    test_stack(10000).await;

    info!("test shared stack case success!");
}

async fn test_stack(count: usize) {
    let device_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    
    for _ in 0..count {
        use_once(device_stack.param().clone()).await;

        async_std::task::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn use_once(mut param: SharedCyfsStackParam) {
    // param.event_type = CyfsStackEventType::None;
    param.requestor_config = CyfsStackRequestorConfig::ws();

    let stack = SharedCyfsStack::open(param).await.unwrap();
    stack.wait_online(None).await.unwrap();

    let req = UtilGetDeviceStaticInfoRequest::new();
    let resp = stack.util().get_device_static_info(req).await.unwrap();
    info!("{}", resp);

    let ret = stack.router_handlers().put_object().add_handler(
        RouterHandlerChain::PreRouter,
        "put-object1",
        0,
        Some("*".to_owned()),
        None,
        RouterHandlerAction::Default,
        None,
    ).await;
    assert!(ret.is_ok());

    stack.stop().await;
}