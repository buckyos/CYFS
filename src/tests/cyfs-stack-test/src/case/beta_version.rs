use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;
use zone_simulator::*;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage test storage dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

pub async fn test() {
    let user1_stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_device1_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let user1_device2_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    let user2_stack = TestLoader::get_shared_stack(DeviceIndex::User2OOD);
    let user2_device1_stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);
    let user2_device2_stack = TestLoader::get_shared_stack(DeviceIndex::User2Device2);

    test_beta_version(&user1_stack, &user1_device1_stack, &user1_device2_stack, &user2_stack, &user2_device1_stack, &user2_device2_stack).await;
}

async fn test_beta_version(
    user1_stack: &SharedCyfsStack,
    user1_device1_stack: &SharedCyfsStack, 
    user1_device2_stack: &SharedCyfsStack, 
    user2_stack: &SharedCyfsStack, 
    user2_device1_stack: &SharedCyfsStack, 
    user2_device2_stack: &SharedCyfsStack) {

    //test_root_state_cross_dec(&user1_stack, &user1_device1_stack, &user1_device2_stack, &user2_stack, &user2_device1_stack, &user2_device2_stack).await;
    test_non_object_req_path().await;
}

// 测试root-state的同zone的跨dec操作 需要配合权限
async fn test_root_state_cross_dec(
    user1_stack: &SharedCyfsStack,
    user1_device1_stack: &SharedCyfsStack, 
    _user1_device2_stack: &SharedCyfsStack, 
    _user2_stack: &SharedCyfsStack, 
    _user2_device1_stack: &SharedCyfsStack, 
    _user2_device2_stack: &SharedCyfsStack) {

    let root_state = user1_stack.root_state_stub(None, Some(user1_device1_stack.dec_id().unwrap().to_owned()));
    let root_info = root_state.get_current_root().await.unwrap();
    info!("current root: {:?}", root_info);

    let access = RootStateOpEnvAccess::new("/", AccessPermissions::Full);
    let op_env = root_state.create_path_op_env_with_access(Some(access)).await.unwrap();

    
    let x1_value = ObjectId::from_base58("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x2_value = ObjectId::from_base58("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();
    
    // test create_new
    op_env.remove_with_path("/new", None).await.unwrap();
    op_env
        .create_new_with_path("/new/a", ObjectMapSimpleContentType::Map)
        .await
        .unwrap();
    op_env
        .create_new_with_path("/new/c", ObjectMapSimpleContentType::Set)
        .await
        .unwrap();

    if let Err(e) = op_env
        .create_new_with_path("/new/a", ObjectMapSimpleContentType::Map)
        .await
    {
        assert!(e.code() == BuckyErrorCode::AlreadyExists);
    } else {
        unreachable!();
    }

    if let Err(e) = op_env
        .create_new_with_path("/new/c", ObjectMapSimpleContentType::Map)
        .await
    {
        assert!(e.code() == BuckyErrorCode::AlreadyExists);
    } else {
        unreachable!();
    }

    // 首先移除老的值，如果存在的话
    op_env.remove_with_path("/x/b", None).await.unwrap();

    let ret = op_env.get_by_path("/x/b").await.unwrap();
    assert_eq!(ret, None);
    let ret = op_env.get_by_path("/x/b/c").await.unwrap();
    assert_eq!(ret, None);

    op_env
        .insert_with_key("/x/b", "c", &x1_value)
        .await
        .unwrap();

    let ret = op_env.get_by_path("/x/b/c").await.unwrap();
    assert_eq!(ret, Some(x1_value));

    let ret = op_env.remove_with_path("/x/b/d", None).await.unwrap();
    assert_eq!(ret, None);

    let root = op_env.commit().await.unwrap();
    info!("new dec root is: {:?}", root);

    {
        let op_env = root_state.create_path_op_env().await.unwrap();
        op_env.remove_with_path("/set", None).await.unwrap();

        let ret = op_env.insert("/set/a", &x2_value).await.unwrap();
        assert!(ret);

        let ret = op_env.contains("/set/a", &x1_value).await.unwrap();
        assert!(!ret);

        let ret = op_env.insert("/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let ret = op_env.insert("/set/a", &x1_value).await.unwrap();
        assert!(!ret);

        let ret = op_env.remove("/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let ret = op_env.insert("/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let root = op_env.commit().await.unwrap();
        info!("new dec root is: {:?}", root);
    }

    info!("test root_state complete!");

}


fn new_object(dec_id: &ObjectId, id: &str) -> Text {
    Text::build(id, "test_header", "hello!")
        .no_create_time()
        .dec_id(dec_id.to_owned())
        .build()
}

fn gen_text_object_list(dec_id: &ObjectId,) -> Vec<(Text,ObjectId)> {
    let mut list = vec![];

    let object = new_object(dec_id, "first-text");
    let object_id = object.text_id().object_id().to_owned();
    list.push((object, object_id));

    let object = new_object(dec_id, "second-text");
    let object_id = object.text_id().object_id().to_owned();
    list.push((object, object_id));

    list
}

async fn clear_all(dec_id: &ObjectId) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);

    let device1 = stack.local_device_id();
    let device2 = TestLoader::get_shared_stack(DeviceIndex::User1Device2).local_device_id();
    let ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD).local_device_id();

    let list= gen_text_object_list(dec_id);
    for (_, object_id) in list {
        info!("will clear object={}, dec={}, target={}", object_id, dec_id, device1);
        test_delete_object(&object_id, dec_id, &stack, &device1).await;

        info!("will clear object={}, dec={}, target={}", object_id, dec_id, device2);
        test_delete_object(&object_id, dec_id, &stack, &device2).await;

        info!("will clear object={}, dec={}, target={}", object_id, dec_id, ood);
        test_delete_object(&object_id, dec_id, &stack, &ood).await;
    }
}

// 跨zone 调用req_path
async fn test_non_object_req_path() {
    let dec_id = new_dec("test-non");
    //clear_all(&dec_id).await;

    async_std::task::spawn(async move {
        loop {
            let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
            let dec_id = new_dec("test-non");
            test_put_object(&dec_id, &stack).await;

            async_std::task::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    // test_outer_put_dec(&dec_id).await;

    let dec_id = new_dec("test-non");
    let target;
    {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
        // stack.wait_online(None).await.unwrap();
        target = stack.local_device_id();
        test_put_object(&dec_id, &stack).await;
    }

    {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
        test_get_object(&dec_id, &stack, &target).await;

        let object = new_object(&dec_id, "first-text");
        let object_id = object.text_id().object_id().to_owned();

        test_delete_object(&object_id, &dec_id, &stack, &target).await;

    }

    info!("test all non case success!");
}

async fn open_access(stack: &SharedCyfsStack, dec_id: &ObjectId) {
    // 开启权限acl
    let meta = stack.root_state_meta_stub(None, None);
    // 为当前Zone内的desc_id开放req_path的读写权限
    let item = GlobalStatePathAccessItem {
        path: "/root/shared".to_owned(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            zone_category: Some(DeviceZoneCategory::CurrentZone),
            dec: Some(dec_id.clone()),
            access: AccessPermissions::Full as u8,
        }),
    };

    meta.add_access(item).await.unwrap();

}

// object层 跨dec 在设置和不设置对应group情况下的操作是否正常
async fn test_outer_put_dec(dec_id: &ObjectId) {

    let dec_id = TestLoader::get_shared_stack(DeviceIndex::User1Device2).dec_id().unwrap().to_owned();
    let (q, _a) = qa_pair();
    let object_id = q.text_id().object_id().to_owned();

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let target_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    let mut req =
        NONPutObjectOutputRequest::new_router(None, object_id.clone(), q.to_vec().unwrap());
    req.common.dec_id = Some(TestLoader::get_shared_stack(DeviceIndex::User1Device1).dec_id().unwrap().to_owned());
    req.common.target = Some(stack.local_device_id().into());
    // req_path 统一格式
    let req_path = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: None,
        req_path: Some("/root/shared".to_owned()),
    };

    let req_path = req_path.format_string();
    println!("req_path: {}", req_path);

    req.common.req_path = Some(req_path);

    // object 层 add_access
    // let mut access = AccessString::new(0);

    // access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    // access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    // access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Write);

    // access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    // access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    // access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Write);

    // access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
    // access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Call);
    // access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Call);

    req.access = None;  // object层的权限

    // 目标req_path层, dec-id开启对应的权限才可以操作
    //open_access(&stack, &dec_id).await;
    //open_access(&target_stack, &dec_id).await;

    let ret = stack.non_service().put_object(req).await;
    match ret {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        }
        Ok(ret) => info!("put: {}", ret),
    }


    let filter = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: None,
        req_path: Some("/root/shared".to_owned()),
    };

    let listener = OnGetObject {};
    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PreRouter,
            "get-object1",
            0,
            Some(filter.format_string()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    let (q, a) = qa_pair();
    //let object_id = q.text_id().object_id().to_owned();

    let mut req = NONGetObjectOutputRequest::new_router(None, object_id, None);
    req.common.dec_id = Some(TestLoader::get_shared_stack(DeviceIndex::User1Device1).dec_id().unwrap().to_owned());
    req.common.target = Some(stack.local_device_id().into());
    // req_path 统一格式
    let req_path = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: None,
        req_path: Some("/root/shared".to_owned()),
    };

    let req_path = req_path.format_string();
    println!("req_path: {}", req_path);

    req.common.req_path = Some(req_path);

    let ret = stack.non_service().get_object(req).await;
    let resp = ret.unwrap();

    let t = Text::clone_from_slice(&resp.object.object_raw).unwrap();
    assert_eq!(*t.text_id().object_id(), *a.text_id().object_id());
    assert_eq!(resp.object.object_id, *a.text_id().object_id());

}

// object层 跨zone在设置和不设置对应group情况下的操作是否正常
async fn test_outer_put_zone(dec_id: &ObjectId) {

    let dec_id = TestLoader::get_shared_stack(DeviceIndex::User2Device2).dec_id().unwrap().to_owned();
    let (q, _a) = qa_pair();
    let object_id = q.text_id().object_id().to_owned();

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let target_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    let mut req =
        NONPutObjectOutputRequest::new_router(None, object_id.clone(), q.to_vec().unwrap());
    req.common.dec_id = Some(TestLoader::get_shared_stack(DeviceIndex::User1Device1).dec_id().unwrap().to_owned());
    req.common.target = Some(target_stack.local_device_id().into());
    // req_path 统一格式
    let req_path = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: Some(dec_id.clone()),
        req_path: Some("/root/shared".to_owned()),
    };

    let req_path = req_path.format_string();
    println!("req_path: {}", req_path);

    req.common.req_path = Some(req_path);

    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Read);

    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Write);

    req.access = Some(access);  // object层的权限

    // 目标req_path层, dec-id开启对应的权限才可以操作
    open_access(&target_stack, &dec_id).await;

    let ret = stack.non_service().put_object(req).await;
    match ret {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        }
        Ok(ret) => info!("{}", ret),
    }
}


fn qa_pair() -> (Text, Text) {
    let q = Text::build("question121111", "test_header1212122", "hello1!21321@")
        .no_create_time()
        .build();
    let a = Text::build("answ321321er1", "test_header132132", "world32131!@")
        .no_create_time()
        .build();

    (q, a)
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

async fn test_delete_object(
    object_id: &ObjectId,
    dec_id: &ObjectId,
    stack: &SharedCyfsStack,
    target: &DeviceId,
) {
    let mut req = NONDeleteObjectOutputRequest::new_router(
        Some(target.object_id().to_owned()),
        object_id.to_owned(),
        None,
    );
    req.common.dec_id = Some(dec_id.to_owned());

    req.common.target = Some(target.object_id().to_owned());
    let _resp = stack.non_service().delete_object(req).await.unwrap();
    info!("delete object success! {}", object_id);
}

async fn test_put_object(dec_id: &ObjectId, stack: &SharedCyfsStack) {
    {
        let object = new_object(dec_id, "first-text");
        let object_id = object.text_id().object_id().to_owned();

        info!("hah1: {}", object_id.to_string());

        info!("will test put object to ood: {}", object_id);

        let mut req = NONPutObjectOutputRequest::new_router(
            None,
            object_id.clone(),
            object.to_vec().unwrap(),
        );

        //req.common.dec_id = Some(TestLoader::get_shared_stack(DeviceIndex::User2Device2).dec_id().unwrap().to_owned());
        req.common.dec_id = Some(dec_id.clone());
        // req_path 统一格式
        let req_path = RequestGlobalStatePath {
            global_state_category: None,
            global_state_root: None,
            dec_id: None,
            req_path: Some("/root/shared".to_owned()),
        };

        let req_path = req_path.format_string();
        println!("req_path: {}", req_path);

        req.common.req_path = Some(req_path);
        //req.common.req_path = Some("/root/shared".to_owned());
        // 权限位操作
        // let mut access = AccessString::new(0);
        // access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
        // access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
        // access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Read);
    
        // access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
        // access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
        // access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Write);

        //req.access = Some(access);
        // 这里是object层对象
        req.access = None;

        // req_path层的权限
        open_access(&stack, dec_id).await;

        let ret = stack.non_service().put_object(req).await.unwrap();
        match ret.result {
            NONPutObjectResult::Accept => {
                info!("first put_object success! {}", object_id);
            }
            NONPutObjectResult::Updated => {
                info!("updated put_object success! {}", object_id);
            }
            NONPutObjectResult::AlreadyExists => {
                info!("put_object but already exists! {}", object_id);
            }
            _ => {
                unreachable!();
            }
        }
    }

    {
        let object = new_object(dec_id, "second-text");
        let object_id = object.text_id().object_id().to_owned();

        let mut req =
            NONPutObjectOutputRequest::new_router(None, object_id, object.to_vec().unwrap());
        req.common.dec_id = Some(dec_id.clone());

        let ret = stack.non_service().put_object(req).await.unwrap();
        match ret.result {
            NONPutObjectResult::Accept => {
                info!("first put_object success! {}", object_id);
            }
            NONPutObjectResult::Updated => {
                info!("updated put_object success! {}", object_id);
            }
            NONPutObjectResult::AlreadyExists => {
                info!("put_object but already exists! {}", object_id);
            }
            _ => {
                unreachable!();
            }
        }
    }
}

async fn test_get_object(dec_id: &ObjectId, stack: &SharedCyfsStack, target: &DeviceId) {

    let filter = format!("dec_id == {}", dec_id);

    // req_path 统一格式
    let req_path = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: Some(dec_id.clone()),
        req_path: Some("/root/shared".to_owned()),
    };

    let req_path = req_path.format_string();
    println!("req_path: {}", req_path);

    let listener = OnGetObject {};
    // stack
    //     .router_handlers()
    //     .add_handler(
    //         RouterHandlerChain::PreRouter,
    //         "get-object1",
    //         0,
    //         Some(filter.clone()),
    //         Some(req_path),
    //         RouterHandlerAction::Default,
    //         Some(Box::new(listener)),
    //     )
    //     .unwrap();

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    let object = new_object(dec_id, "first-text");
    let object_id = object.text_id().object_id().to_owned();

    info!("hah: {}", object_id.to_string());
    let mut req = NONGetObjectOutputRequest::new_router(None, object_id, None);
    req.common.dec_id = Some(dec_id.clone());
    req.common.target = Some(target.object_id().to_owned());
    // req_path 统一格式
    let req_path = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: None,
        req_path: Some("/root/shared".to_owned()),
    };

    let req_path = req_path.format_string();
    req.common.req_path = Some(req_path);

    let ret = stack.non_service().get_object(req).await;
    let resp = ret.unwrap();

    info!("test_get_object: {}", resp);

    /*
    // used for clear old data
    for item in &resp.objects {
        test_delete_object(&item.object.as_ref().unwrap().object_id, dec_id, stack, target).await;
    }
    */
}