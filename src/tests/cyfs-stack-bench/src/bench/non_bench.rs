use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use cyfs_stack_loader::CyfsServiceLoader;
use crate::{Bench, BenchEnv};
use log::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;
use zone_simulator::*;

pub struct NONBench {}

#[async_trait]
impl Bench for NONBench {
    async fn bench(&self, env: BenchEnv, _ood_path: String, t: u64) -> bool {
        let ret = if env == BenchEnv::Simulator {
            let time = std::time::Instant::now();
            static COUNT: AtomicU64 = AtomicU64::new(0);
            //for _ in 0..t {
                let ret = test().await;
            //}
    
            let tps = if time.elapsed().as_secs() > 0 { t / time.elapsed().as_secs() } else { 0 };
        
            info!("non bench TPS: {}/{} = {}", t, time.elapsed().as_secs(), tps);

            true
        } else {
            // Test Code
            let id = "5aSixgNAmFYV4vgRk1CQeQmhG9532dtKMUMUfJYasE1n".to_string();
            let stack = CyfsServiceLoader::shared_cyfs_stack(Some(&id));
            let dec_id = ObjectId::from_base58("9tGpLNnDpa8deXEk2NaWGccEu4yFQ2DrTZJPLYLT7gj4").unwrap();
            //let user1_stack = stack.fork_with_new_dec(Some(dec_id.clone())).await.unwrap();
            //user1_stack.wait_online(None).await.unwrap();

            let root_state = stack.root_state_stub(None, Some(dec_id));
            let root_info = root_state.get_current_root().await.unwrap();
            info!("current root: {:?}", root_info);

            {
                // create_path_op_env None access默认权限操作自己dec_id
                let op_env = root_state.create_path_op_env().await.unwrap();
                op_env.remove_with_path("/set", None).await.unwrap();

                let x1_value = ObjectId::from_base58("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
                let x2_value = ObjectId::from_base58("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();

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
                
            true
        };

        ret
    }

    fn name(&self) -> &str {
        "NON Bench"
    }
}


fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage test storage dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

pub async fn test() -> bool {
    test_non_object_req_path().await
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
async fn test_non_object_req_path() -> bool {
    let dec_id = new_dec("test-non");
    let user1_stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_stack = user1_stack.fork_with_new_dec(Some(dec_id.clone())).await.unwrap();
    user1_stack.wait_online(None).await.unwrap();

    async_std::task::spawn(async move {
        loop {
            let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
            let dec_id = new_dec("test-non");
            test_put_object(&dec_id, &stack).await;
            async_std::task::sleep(std::time::Duration::from_secs(60)).await;
        }
    });

    test_outer_put_dec(&dec_id).await;

    {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
        let target = stack.local_device_id();
        test_get_object(&dec_id, &stack, &target).await;

        let object = new_object(&dec_id, "first-text");
        let object_id = object.text_id().object_id().to_owned();

        test_delete_object(&object_id, &dec_id, &stack, &target).await;
    }
    info!("test all non case success!");

    true
}

async fn open_access(stack: &SharedCyfsStack, dec_id: &ObjectId, req_path: impl Into<String>, _perm: AccessPermissions) {
    // 开启权限 rmeta, 为当前Zone内的dec_id开放req_path的读写权限
    let meta = stack.root_state_meta_stub(None, None);
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Write);

    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Read);

    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Call);
    
    access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Read);


    let item = GlobalStatePathAccessItem {
        path: req_path.into(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };
    meta.add_access(item).await.unwrap();

}

// object层 跨dec 在设置和不设置对应group情况下的操作是否正常
// object层 跨zone在设置和不设置对应group情况下的操作是否正常, 不允许跨zone put, 允许跨zone get
async fn test_outer_put_dec(_dec_id: &ObjectId) {

    let dec_id = TestLoader::get_shared_stack(DeviceIndex::User1Device2).dec_id().unwrap().to_owned();
    let (_q, a) = qa_pair();
    let object_id = a.text_id().object_id().to_owned();

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let target_stack = TestLoader::get_shared_stack(DeviceIndex::User2Device2);

    let mut req =
        NONPutObjectOutputRequest::new_router(None, object_id.clone(), a.to_vec().unwrap());
    req.common.dec_id = Some(TestLoader::get_shared_stack(DeviceIndex::User1Device1).dec_id().unwrap().to_owned());
    req.common.target = Some(stack.local_device_id().into());
    // req_path 统一格式, put_object 一般不需要req_path
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
    //open_access(&stack, dec_id, "/root/shared", AccessPermissions::Full).await;
    //open_access(&target_stack, dec_id, "/root/shared", AccessPermissions::Full).await;

    let target_dec_id = TestLoader::get_shared_stack(DeviceIndex::User1Device1).dec_id().unwrap().clone();
    let req_path = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: Some(target_dec_id.to_owned()),
        req_path: Some("/root/shared".to_owned()),
    };

    req.common.req_path = Some(req_path.format_string());

    let ret = stack.non_service().put_object(req).await;
    match ret {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        }
        Ok(ret) => info!("put: {}", ret),
    }

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    let (_q, a) = qa_pair();
    //let object_id = q.text_id().object_id().to_owned();

    let mut req = NONGetObjectOutputRequest::new_router(None, object_id, None);
    req.common.dec_id = Some(TestLoader::get_shared_stack(DeviceIndex::User1Device1).dec_id().unwrap().to_owned());
    req.common.target = Some(stack.local_device_id().into());

    let new_dec = new_dec("cross_dec");
    let stack1 = stack.fork_with_new_dec(Some(new_dec.clone())).await.unwrap();
    stack1.wait_online(None).await.unwrap();
    let ret = stack1.non_service().get_object(req.clone()).await;
    let resp = ret.unwrap();
    // cross dec
    let t = Text::clone_from_slice(&resp.object.object_raw).unwrap();
    assert_eq!(*t.text_id().object_id(), *a.text_id().object_id());
    assert_eq!(resp.object.object_id, *a.text_id().object_id());

    // cross zone not perm
    // let ret = target_stack.non_service().get_object(req.clone()).await;
    // assert!(ret.is_err());
    // let err = ret.err().unwrap();
    // assert_eq!(err.code(), BuckyErrorCode::PermissionDenied);

    // cross zone add perm
    // 目标req_path层, dec-id开启对应的权限才可以操作
    open_access(&stack, &TestLoader::get_shared_stack(DeviceIndex::User1Device1).dec_id().unwrap().to_owned(), "/root/shared", AccessPermissions::Full).await;
    // 挂在树上
    let stub = stack.root_state_stub(Some(stack.local_device_id().object_id().to_owned()), None);
    let op_env = stub.create_path_op_env().await.unwrap();
    op_env.set_with_path("/root/shared", &object_id, None, true).await.unwrap();
    let _root_info = op_env.commit().await.unwrap();

    req.common.req_path = Some(req_path.format_string());
    req.common.dec_id = Some(TestLoader::get_shared_stack(DeviceIndex::User1Device1).dec_id().unwrap().to_owned());
    req.common.target = Some(stack.local_device_id().into());
    let ret = target_stack.non_service().get_object(req.clone()).await;
    let resp = ret.unwrap();
    let t = Text::clone_from_slice(&resp.object.object_raw).unwrap();
    assert_eq!(*t.text_id().object_id(), *a.text_id().object_id());
    assert_eq!(resp.object.object_id, *a.text_id().object_id());

    info!("cross zone get object success");

}

fn qa_pair() -> (Text, Text) {
    let q = Text::build("question", "test_header", "hello")
        .no_create_time()
        .build();
    let a = Text::build("answer", "test_header", "world")
        .no_create_time()
        .build();

    (q, a)
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

        info!("will test put object to ood: {}", object_id);

        let mut req = NONPutObjectOutputRequest::new_router(
            None,
            object_id.clone(),
            object.to_vec().unwrap(),
        );

        req.common.dec_id = Some(dec_id.clone());
        req.common.target = Some(stack.local_device_id().object_id().to_owned());
        // req_path 统一格式
        let req_path = RequestGlobalStatePath {
            global_state_category: None,
            global_state_root: None,
            dec_id: None,
            req_path: Some("/root/shared".to_owned()),
        };

        let req_path = req_path.format_string();

        req.common.req_path = Some(req_path);
        // 权限位操作
        let mut access = AccessString::default();
        access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Read);
        access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Write);
        access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Call);
        // 这里是object层对象
        req.access = Some(access);

        // req_path层的权限
        open_access(&stack, dec_id, "/root/shared", AccessPermissions::Full).await;

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
    let get_object_path = "/.cyfs/api/handler/pre_router/get_object/";
    // req_path 统一格式
    let req_path = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: Some(dec_id.clone()),
        req_path: Some(get_object_path.to_owned()),
    };

    let req_path = req_path.format_string();
    debug!("haha: {}", req_path.to_owned());

    // req_path层的权限
    let target_stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    open_access(&target_stack, dec_id, get_object_path.to_owned(), AccessPermissions::Full).await;
    open_access(&stack, dec_id, get_object_path.to_owned(), AccessPermissions::Full).await;

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    let object = new_object(dec_id, "first-text");
    let object_id = object.text_id().object_id().to_owned();

    let mut req = NONGetObjectOutputRequest::new_router(None, object_id, None);
    req.common.dec_id = Some(dec_id.clone());
    req.common.target = Some(target_stack.local_device_id().object_id().to_owned());
    // req_path 统一格式
    req.common.req_path = Some(req_path.to_owned());

    let ret = stack.non_service().get_object(req).await;
    let resp = ret.unwrap();

    info!("test_get_object: {}", resp);
}