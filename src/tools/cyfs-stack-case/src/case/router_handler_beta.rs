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
    
    //let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2 = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    put_object(&device1, &device2).await;
    get_object(&device1, &device2).await;

    post_object(&device1, &device2).await;

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


async fn open_hook_access(stack: &SharedCyfsStack) {
    // 需要使用system-dec身份操作
    let dec_id = stack.dec_id().unwrap().to_owned();

    let system_stack = stack
        .fork_with_new_dec(Some(cyfs_core::get_system_dec_app().to_owned()))
        .await
        .unwrap();
    system_stack.wait_online(None).await.unwrap();

    // 开启权限，需要修改system's rmeta
    let meta = system_stack.root_state_meta_stub(None, None);
    /*
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Call);
    */
    let item = GlobalStatePathAccessItem {
        path: CYFS_HANDLER_VIRTUAL_PATH.to_owned(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            zone_category: Some(DeviceZoneCategory::CurrentZone),
            dec: Some(dec_id.clone()),
            access: AccessPermissions::WriteOnly as u8,
        }),
    };

    meta.add_access(item).await.unwrap();
}


async fn put_object(device1: &SharedCyfsStack, device2: &SharedCyfsStack) {
    device1
        .root_state_meta_stub(None, None)
        .clear_access()
        .await
        .unwrap();
    device2
        .root_state_meta_stub(None, None)
        .clear_access()
        .await
        .unwrap();

    let dec1 = device1.dec_id().unwrap();
    let call_path = "/test1";

    // open access for self dec's register post-object handler
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Write);
    
    let item = GlobalStatePathAccessItem {
        path: call_path.to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };

    device2
        .root_state_meta_stub(None, None)
        .add_access(item)
        .await
        .unwrap();

    // system 开启hook权限
    open_hook_access(device2).await;

    // 添加一个处理器
    let listener = OnPutObject {};
    let req_path = RequestGlobalStatePath::new(Some(dec1.clone()), Some(call_path.to_owned()));
    let ret = device2.router_handlers().add_handler(
        RouterHandlerChain::PreRouter,
        "put-object1",
        0,
        None,
        Some(req_path.to_string()),
        RouterHandlerAction::Default,
        Some(Box::new(listener)),
    );
    assert!(ret.is_ok());

    // 添加一个观察者
    let listener = OnPutObjectWatcher {};
    let ret = device2.router_handlers().add_handler(
        RouterHandlerChain::PreRouter,
        "watch-put_object1",
        // 观察者模式使用负数索引
        -1,
        None,
        Some(req_path.to_string()),
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

        let mut req = NONPutObjectOutputRequest::new_router(Some(device2.local_device_id().object_id().to_owned()), object_id, q.to_vec().unwrap());
        req.common.dec_id = Some(dec1.clone());
        // let mut req = NONPutObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, q.to_vec().unwrap());
        // req.common.dec_id = Some(dec1.clone());

        //req.common.req_path = Some(req_path.to_string());
        let ret = device1.non_service().put_object(req).await;

        match ret {
            Ok(resp) => {
                info!("put_object success! object_id={}, resp={}", object_id, resp);
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

        let mut req = NONPutObjectOutputRequest::new_router(Some(device2.local_device_id().object_id().to_owned()), object_id, q.to_vec().unwrap());
        req.common.dec_id = Some(dec1.clone());
        //req.common.req_path = Some(req_path.to_string());
        let ret = device1.non_service().put_object(req).await;
        match ret {
            Ok(resp) => {
                error!(
                    "put_object but success! object_id={}, resp={}",
                    object_id, resp
                );
            }
            Err(e) => {
                info!("put_object failed! object_id={}, {}", object_id, e);
                assert_eq!(e.code(), BuckyErrorCode::NotSupport);
            }
        }
    }
}

async fn get_object(device1: &SharedCyfsStack, device2: &SharedCyfsStack) {
    device1
        .root_state_meta_stub(None, None)
        .clear_access()
        .await
        .unwrap();
    device2
        .root_state_meta_stub(None, None)
        .clear_access()
        .await
        .unwrap();

    let dec1 = device1.dec_id().unwrap();
    let call_path = "/test2";

    // open access for self dec's register post-object handler
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Write);

    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Read);

    let item = GlobalStatePathAccessItem {
        path: call_path.to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };
    device2
        .root_state_meta_stub(None, None)
        .add_access(item)
        .await
        .unwrap();

    // system 开启hook权限
    open_hook_access(device2).await;

    let listener = OnGetObject {};
    let req_path = RequestGlobalStatePath::new(Some(dec1.clone()), Some(call_path.to_owned()));
    device2
        .router_handlers()
        .add_handler(
            RouterHandlerChain::Handler,
            "get-object1",
            0,
            None,
            Some(req_path.to_string()),
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    let (q, _a) = qa_pair();
    let object_id = q.text_id().object_id().to_owned();

    let mut req = NONGetObjectOutputRequest::new_router(Some(device2.local_device_id().object_id().to_owned()), object_id, None);
    req.common.dec_id = Some(dec1.clone());
    //req.common.req_path = Some(req_path.to_string());

    let ret = device1.non_service().get_object(req).await;
    let resp = ret.unwrap();

    let t = Text::clone_from_slice(&resp.object.object_raw).unwrap();
    assert_eq!(*t.text_id().object_id(), *q.text_id().object_id());
    assert_eq!(resp.object.object_id, *q.text_id().object_id());
}

async fn post_object(device1: &SharedCyfsStack, device2: &SharedCyfsStack) {
    device1
        .root_state_meta_stub(None, None)
        .clear_access()
        .await
        .unwrap();
    device2
        .root_state_meta_stub(None, None)
        .clear_access()
        .await
        .unwrap();

    let dec1 = device1.dec_id().unwrap();
    let call_path = "/test3";

    // open access for self dec's register post-object handler
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Write);
    let item = GlobalStatePathAccessItem {
        path: call_path.to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };
    device2
        .root_state_meta_stub(None, None)
        .add_access(item)
        .await
        .unwrap();

    // 同dec的不同设备注册handler
    let req_path = RequestGlobalStatePath::new(Some(dec1.clone()), Some(call_path.to_owned()));
    device2.router_handlers().post_object().add_handler(
        RouterHandlerChain::Handler,
        "must_same_source_target_dec",
        0,
        None,
        Some(req_path.to_string()),
        RouterHandlerAction::Default,
        Some(Box::new(OnPostObject { })),
    ).await.unwrap();

    // post_object (device1, dec1) -> (decvice2, dec1) 
    let (q, a) = qa_pair();
    let object_id = q.text_id().object_id().to_owned();

    let mut req = NONPostObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, q.to_vec().unwrap());
    req.common.dec_id = Some(dec1.clone());


    let req_path = RequestGlobalStatePath::new(Some(dec1.clone()), Some(call_path.to_owned()));
    req.common.req_path = Some(req_path.to_string());

    let ret = device1.non_service().post_object(req.clone()).await;
    assert!(ret.is_ok());
    let resp = ret.unwrap();

    let t = Text::clone_from_slice(&resp.object.unwrap().object_raw).unwrap();
    assert_eq!(*t.text_id().object_id(), *a.text_id().object_id());
}