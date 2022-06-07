use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;
use zone_simulator::*;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!("generage dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

fn qa_pair() -> (Text, Text) {
    let q = Text::build("question", "test_header", "hello!")
        .no_create_time()
        .build();
    let a = Text::build("answer", "test_header", "world!")
        .no_create_time()
        .build();

    (q, a)
}

pub async fn test() {
    let dec_id = new_dec("user1");

    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    put_object(&user1_ood, &dec_id).await;
    get_object(&user1_ood, &dec_id).await;

    let user2_ood = TestLoader::get_shared_stack(DeviceIndex::User2OOD);
    put_object(&user2_ood, &dec_id).await;
    get_object(&user2_ood, &dec_id).await;

    info!("test all router handler case success!")
}

struct OnPutObjectWatcher;

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult>
    for OnPutObjectWatcher
{
    async fn call(
        &self,
        param: &RouterHandlerPutObjectRequest,
    ) -> BuckyResult<RouterHandlerPutObjectResult> {
        info!("watch_put_object: {}", param.request.object.object_id);

        let result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnPutObject;

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult>
    for OnPutObject
{
    async fn call(
        &self,
        param: &RouterHandlerPutObjectRequest,
    ) -> BuckyResult<RouterHandlerPutObjectResult> {
        info!("handler_put_object: {}", param.request.object.object_id);

        let (q, _a) = qa_pair();

        let object = Text::clone_from_slice(&param.request.object.object_raw).unwrap();
        let result = if *object.text_id().object_id() == *q.text_id().object_id() {
            let response = NONPutObjectInputResponse {
                result: NONPutObjectResult::Accept,
                object_expires_time: None,
                object_update_time: None,
            };

            // 使用answer对象应答
            RouterHandlerPutObjectResult {
                action: RouterHandlerAction::Response,
                request: None,
                response: Some(Ok(response)),
            }
        } else {
            // 其余对象，直接返回NotSupport
            let msg = format!(
                "put object id not support! req={}",
                param.request.object.object_id
            );
            warn!("{}", msg);
            let response = Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));

            RouterHandlerPutObjectResult {
                action: RouterHandlerAction::Response,
                request: None,
                response: Some(response),
            }
        };

        Ok(result)
    }
}

struct OnPostObject;

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for OnPostObject
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        info!("handler_post_object: {}", param.request.object.object_id);

        let (q, a) = qa_pair();

        let object = Text::clone_from_slice(&param.request.object.object_raw).unwrap();
        let result = if *object.text_id().object_id() == *q.text_id().object_id() {
            let response = NONPostObjectInputResponse {
                object: Some(NONObjectInfo::new(
                    a.text_id().object_id().to_owned(),
                    a.to_vec().unwrap(),
                    None,
                )),
            };

            // 使用answer对象应答
            RouterHandlerPostObjectResult {
                action: RouterHandlerAction::Response,
                request: None,
                response: Some(Ok(response)),
            }
        } else {
            let msg = format!(
                "post object id not support! req={}",
                param.request.object.object_id
            );
            warn!("{}", msg);
            let response = Err(BuckyError::new(BuckyErrorCode::NotFound, msg));

            // 其余对象，直接返回
            RouterHandlerPostObjectResult {
                action: RouterHandlerAction::Response,
                request: None,
                response: Some(response),
            }
        };

        Ok(result)
    }
}

struct OnGetObject;

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerGetObjectRequest, RouterHandlerGetObjectResult>
    for OnGetObject
{
    async fn call(
        &self,
        param: &RouterHandlerGetObjectRequest,
    ) -> BuckyResult<RouterHandlerGetObjectResult> {
        info!("handler_get_object: {}", param.request.object_id);

        let (q, a) = qa_pair();

        assert!(*q.text_id().object_id() == param.request.object_id);

        let object_raw = a.to_vec().unwrap();
        info!("will return a: {:?}", object_raw);

        let mut response =
            NONGetObjectInputResponse::new(a.text_id().object_id().to_owned(), object_raw, None);
        response.init_times()?;

        // let object = Text::clone_from_slice(&param.object_raw).unwrap();
        let result = RouterHandlerGetObjectResult {
            action: RouterHandlerAction::Response,
            request: None,
            response: Some(Ok(response)),
        };

        Ok(result)
    }
}

async fn put_object(stack: &SharedCyfsStack, dec_id: &ObjectId) {
    //let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    //let user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let filter = format!("dec_id == {}", dec_id);

    // 添加一个处理器
    let listener = OnPutObject {};
    let ret = stack.router_handlers().add_handler(
        RouterHandlerChain::PreRouter,
        "put-object1",
        0,
        &filter,
        RouterHandlerAction::Default,
        Some(Box::new(listener)),
    );
    assert!(ret.is_ok());

    // 添加一个观察者
    let listener = OnPutObjectWatcher {};
    let ret = stack.router_handlers().add_handler(
        RouterHandlerChain::PreRouter,
        "watch-object1",
        // 观察者模式使用负数索引
        -1,
        &filter,
        RouterHandlerAction::Pass,
        Some(Box::new(listener)),
    );
    assert!(ret.is_ok());

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    // 发起一次成功的put
    {
        let (q, _a) = qa_pair();
        let object_id = q.text_id().object_id().to_owned();

        let mut req = NONPutObjectOutputRequest::new_router(None, object_id, q.to_vec().unwrap());
        req.common.dec_id = Some(dec_id.clone());

        let ret = stack.non_service().put_object(req).await;

        match ret {
            Ok(resp) => {
                info!("put_object success! object_id={}, resp={}", object_id, resp);
                assert_eq!(resp.result, NONPutObjectResult::Accept);
            }
            Err(e) => {
                error!("put_object failed! object_id={}, {}", object_id, e);
                unreachable!();
            }
        }
    }

    // 一次失败的put
    {
        let q = Text::build("simple", "test_header", "hello!")
            .no_create_time()
            .build();
        let object_id = q.text_id().object_id().to_owned();

        let mut req = NONPutObjectOutputRequest::new_router(None, object_id, q.to_vec().unwrap());
        req.common.dec_id = Some(dec_id.clone());

        let ret = stack.non_service().put_object(req).await;
        match ret {
            Ok(resp) => {
                error!(
                    "put_object but success! object_id={}, resp={}",
                    object_id, resp
                );
                unreachable!();
            }
            Err(e) => {
                info!("put_object failed! object_id={}, {}", object_id, e);
                assert_eq!(e.code(), BuckyErrorCode::NotSupport);
            }
        }
    }
}

async fn post_object(stack: &SharedCyfsStack, dec_id: &ObjectId) {
    //let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    //let user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let filter = format!("dec_id == {}", dec_id);

    // 添加一个处理器
    let listener = OnPostObject {};
    let ret = stack.router_handlers().add_handler(
        RouterHandlerChain::PreRouter,
        "post-object1",
        0,
        &filter,
        RouterHandlerAction::Default,
        Some(Box::new(listener)),
    );
    assert!(ret.is_ok());

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    // 发起一次QA
    {
        let (q, a) = qa_pair();
        let object_id = q.text_id().object_id().to_owned();

        let mut req = NONPostObjectOutputRequest::new_router(None, object_id, q.to_vec().unwrap());
        req.common.dec_id = Some(dec_id.clone());

        let ret = stack.non_service().post_object(req).await;

        match ret {
            Ok(resp) => {
                info!(
                    "post_object success! object_id={}, resp={}",
                    object_id, resp
                );
                let resp_object = resp.object.unwrap().object.unwrap();
                assert_eq!(resp_object.object_id(), a.text_id().object_id().to_owned());
            }
            Err(e) => {
                error!("post_object failed! object_id={}, {}", object_id, e);
                unreachable!();
            }
        }
    }

    // 一次未处理的QA
    {
        let q = Text::build("simple", "test_header", "hello!")
            .no_create_time()
            .build();
        let object_id = q.text_id().object_id().to_owned();

        let mut req = NONPostObjectOutputRequest::new_router(None, object_id, q.to_vec().unwrap());
        req.common.dec_id = Some(dec_id.clone());

        let ret = stack.non_service().post_object(req).await;
        match ret {
            Ok(resp) => {
                error!(
                    "post_object but success! object_id={}, resp={}",
                    object_id, resp
                );
                unreachable!();
            }
            Err(e) => {
                info!("post_object but not found! object_id={}, {}", object_id, e);
                assert_eq!(e.code(), BuckyErrorCode::NotFound);
            }
        }
    }
}

async fn get_object(stack: &SharedCyfsStack, dec_id: &ObjectId) {
    let filter = format!("dec_id == {}", dec_id);

    let listener = OnGetObject {};
    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PreRouter,
            "get-object1",
            0,
            &filter,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    let (q, a) = qa_pair();
    let object_id = q.text_id().object_id().to_owned();

    let mut req = NONGetObjectOutputRequest::new_router(None, object_id, None);
    req.common.dec_id = Some(dec_id.clone());

    let ret = stack.non_service().get_object(req).await;
    let resp = ret.unwrap();

    let t = Text::clone_from_slice(&resp.object.object_raw).unwrap();
    assert_eq!(*t.text_id().object_id(), *a.text_id().object_id());
    assert_eq!(resp.object.object_id, *a.text_id().object_id());
}
