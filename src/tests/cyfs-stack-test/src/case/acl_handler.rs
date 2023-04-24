use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;
use zone_simulator::*;

use http_types::Url;

const USER_ACCESS_TOKEN: &str = "123456";

struct OnDynamicAcl;

pub async fn test() {
    test_handler().await;
    
    info!("test all acl handler case success!");

    async_std::task::sleep(std::time::Duration::from_secs(1000)).await;
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerAclRequest, RouterHandlerAclResult> for OnDynamicAcl {
    async fn call(&self, param: &RouterHandlerAclRequest) -> BuckyResult<RouterHandlerAclResult> {
        info!("will handle dynamic acl: {}, query={:?}", param.request.req_path, param.request.req_query_string);

        let mut action = AclAction::Accept;
        let querys: Vec<_> = param.request.req_query_string.as_ref().unwrap().split('&').collect();
        for query in querys {
            if let Some((k, v)) = query.split_once('=') {
                if k == "token" {
                    if v != USER_ACCESS_TOKEN {
                        warn!("invalid user token: {}", v);
                        action = AclAction::Reject;
                    } else {
                        action = AclAction::Accept;
                    }

                    break;
                }
            } else {
                unreachable!("should not come here in out test case! {}", query);
            }
        }

        // let url = Url::parse(&param.request.req_path).unwrap();

        let resp = AclHandlerResponse {
            action,
        };

        let result = RouterHandlerAclResult {
            action: RouterHandlerAction::Response,
            request: None,
            response: Some(Ok(resp)),
        };

        Ok(result)
    }
}

async fn test_handler() {
    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    const TEST_REQ_PATH: &str = "/test/dynamic/call";

    // First set handler access for {req_path} for clean test environment
    device1
        .root_state_meta_stub(None, None)
        .clear_access()
        .await
        .unwrap();

    // Add rmeta 'Handler' access for specific req path 
    let item = GlobalStatePathAccessItem {
        path: TEST_REQ_PATH.to_owned(),
        access: GlobalStatePathGroupAccess::Handler,
    };

    device1
        .root_state_meta_stub(None, None)
        .add_access(item)
        .await
        .unwrap();

    // Add acl handler to handle request
    let handler = OnDynamicAcl {};
    device1.router_handlers().add_handler(
        RouterHandlerChain::Acl,
        "test-dynamic-acl-1",
        0,
        Some("*".to_owned()),
        Some(TEST_REQ_PATH.to_owned()),
        RouterHandlerAction::Reject,
        Some(Box::new(handler)),
    ).unwrap();

    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    // try get object from device1 from /test/dynamic/call as current dec identity
    let target = device1.local_device_id().object_id().to_owned();
    let stub = device2.root_state_accessor_stub(Some(target), None);

    let full_req_path = format!("{}?token={}", TEST_REQ_PATH, USER_ACCESS_TOKEN);

    let resp: Result<NONGetObjectOutputResponse, BuckyError> = stub.get_object_by_path(full_req_path).await;
    match resp {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::NotFound);
            info!("get object from path: {:?}", e);
        }
        Ok(_) => {
            unreachable!();
        }
    }

    let full_req_path = format!("{}?token={}", TEST_REQ_PATH, "12345");

    let resp: Result<NONGetObjectOutputResponse, BuckyError> = stub.get_object_by_path(full_req_path).await;
    match resp {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
            info!("get object from path: {:?}", e);
        }
        Ok(_) => {
            unreachable!();
        }
    }
}
