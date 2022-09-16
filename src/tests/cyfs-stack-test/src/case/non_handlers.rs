use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;
use zone_simulator::*;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage non_handlers dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

struct OnPreNOCPutObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult>
    for OnPreNOCPutObject
{
    async fn call(
        &self,
        param: &RouterHandlerPutObjectRequest,
    ) -> BuckyResult<RouterHandlerPutObjectResult> {
        info!(
            "pre_noc put_object: stack={}, request={}",
            self.stack, param.request
        );
        assert!(param.response.is_none());

        let result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnPostNOCPutObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult>
    for OnPostNOCPutObject
{
    async fn call(
        &self,
        param: &RouterHandlerPutObjectRequest,
    ) -> BuckyResult<RouterHandlerPutObjectResult> {
        assert!(param.response.is_some());

        info!(
            "post_noc put_object: stack={}, request={}, response={:?}",
            self.stack,
            param.request,
            param.response.as_ref().unwrap()
        );
        let result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnPreForwardPutObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult>
    for OnPreForwardPutObject
{
    async fn call(
        &self,
        param: &RouterHandlerPutObjectRequest,
    ) -> BuckyResult<RouterHandlerPutObjectResult> {
        info!(
            "pre_forward put_object: stack={}, request={}",
            self.stack, param.request
        );
        assert!(param.response.is_none());

        let result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnPostForwardPutObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult>
    for OnPostForwardPutObject
{
    async fn call(
        &self,
        param: &RouterHandlerPutObjectRequest,
    ) -> BuckyResult<RouterHandlerPutObjectResult> {
        assert!(param.response.is_some());

        info!(
            "post_forward put_object: stack={}, request={}, response={:?}",
            self.stack,
            param.request,
            param.response.as_ref().unwrap()
        );
        let result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnPreRouterPutObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult>
    for OnPreRouterPutObject
{
    async fn call(
        &self,
        param: &RouterHandlerPutObjectRequest,
    ) -> BuckyResult<RouterHandlerPutObjectResult> {
        info!(
            "pre_router put_object: stack={}, request={}",
            self.stack, param.request
        );
        assert!(param.response.is_none());

        let result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnPostRouterPutObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult>
    for OnPostRouterPutObject
{
    async fn call(
        &self,
        param: &RouterHandlerPutObjectRequest,
    ) -> BuckyResult<RouterHandlerPutObjectResult> {
        assert!(param.response.is_some());

        info!(
            "post_router put_object: stack={}, request={}, response={:?}",
            self.stack,
            param.request,
            param.response.as_ref().unwrap()
        );
        let result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

pub async fn test() {
    let dec_id = new_dec("non-handlers");
    add_handlers_to_all_stacks(&dec_id);

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    // let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    put_object(&user1_device1, &user2_device1, &dec_id).await;
}

fn add_handlers_to_all_stacks(dec_id: &ObjectId) {
    // zone1
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    add_handlers_for_stack("user1_ood", &stack, dec_id);

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    add_handlers_for_stack("user1_device1", &stack, dec_id);

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
    add_handlers_for_stack("user1_device2", &stack, dec_id);

    // zone2
    let stack = TestLoader::get_shared_stack(DeviceIndex::User2OOD);
    add_handlers_for_stack("user2_ood", &stack, dec_id);

    let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);
    add_handlers_for_stack("user2_device1", &stack, dec_id);

    let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device2);
    add_handlers_for_stack("user2_device2", &stack, dec_id);
}

fn add_handlers_for_stack(name: &str, stack: &SharedCyfsStack, dec_id: &ObjectId) {
    let filter = format!("dec_id == {} && protocol != http-local", dec_id);

    // pre_noc
    let listener = OnPreNOCPutObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PreNOC,
            "pre-noc",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // post-noc
    let listener = OnPostNOCPutObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PostNOC,
            "post-noc",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // pre-forward
    let listener = OnPreForwardPutObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PreForward,
            "pre-forward",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // post-forward
    let listener = OnPostForwardPutObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PostForward,
            "post-forward",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // pre-router
    let listener = OnPreRouterPutObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PreRouter,
            "pre-router",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // post-router
    let listener = OnPostRouterPutObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PostRouter,
            "post-router",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();
}

async fn put_object(from: &SharedCyfsStack, to: &SharedCyfsStack, dec_id: &ObjectId) {
    let object = Text::build("question", "test_header", "hello!").build();
    let object_id = object.text_id().object_id().to_owned();

    let target = to.local_device_id();
    let mut req = NONPutObjectOutputRequest::new_router(
        Some(target.into()),
        object_id,
        object.to_vec().unwrap(),
    );
    req.common.dec_id = Some(dec_id.to_owned());
    req.common.req_path = Some("/root/share".to_owned());

    let ret = from.non_service().put_object(req).await;

    match ret {
        Ok(resp) => {
            info!(
                "non put_object success! object_id={}, resp={}",
                object_id, resp
            );
            assert_eq!(resp.result, NONPutObjectResult::Accept);
        }
        Err(e) => {
            error!("non put_object failed! object_id={}, {}", object_id, e);
            unreachable!();
        }
    }
}
