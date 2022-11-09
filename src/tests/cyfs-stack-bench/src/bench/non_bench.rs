use async_trait::async_trait;
use crate::{Bench, BenchEnv, sim_zone::SimZone, Stat};
use log::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use super::constant::*;

pub struct NONBench {}

#[async_trait]
impl Bench for NONBench {
    async fn bench(&self, env: BenchEnv, zone: &SimZone, _ood_path: String, t: u64) -> bool {
        info!("begin test NONBench...");
        let begin = std::time::Instant::now();
        let ret = if env == BenchEnv::Simulator {
            for _ in 0..t {
                let _ret = test(zone).await;
            }
            true
        } else {
            test2().await
        };
        
        let dur = begin.elapsed();
        info!("end test NONBench: {:?}", dur);
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, NON_ALL_IN_ONE, costs).await;
        ret
    }

    fn name(&self) -> &str {
        "NON Bench"
    }
}


fn new_dec(name: &str, zone: &SimZone) -> ObjectId {
    let people_id = zone.get_object_id_by_name("zone1_people");

    let dec_id = DecApp::generate_id(people_id, name);

    info!(
        "generage test storage dec_id={}, people={}",
        dec_id, people_id
    );

    dec_id
}

pub async fn test(zone: &SimZone) -> bool {
    info!("begin test wait_on_line...");
    let begin = std::time::Instant::now();
    let dec_id = new_dec("test-non", zone);
    let user1_stack = zone.get_shared_stack("zone1_ood");
    let user1_stack = user1_stack.fork_with_new_dec(Some(dec_id.clone())).await.unwrap();
    user1_stack.wait_online(None).await.unwrap();

    let dur = begin.elapsed();
    info!("end test wait_on_line: {:?}", dur);

    let stack = zone.get_shared_stack("zone1_device2");
    let dec_id = new_dec("test-non", zone);
    let costs = test_put_object(&dec_id, &stack).await;
    Stat::write(zone, NON_PUT_OBJECT, costs).await;

    let costs = test_outer_put_dec(&dec_id, zone).await;
    Stat::write(zone, NON_PUT_OUTER_OBJECT, costs).await;

    {
        let stack = zone.get_shared_stack("zone1_device2");
        let target = stack.local_device_id();
        let costs = test_get_object(&dec_id, &stack, &target, zone).await;
        Stat::write(zone, NON_GET_OBJECT, costs).await;

        let object = new_object(&dec_id, "first-text");
        let object_id = object.text_id().object_id().to_owned();

        let costs = test_delete_object(&object_id, &dec_id, &stack, &target).await;
        Stat::write(zone, NON_DELETE_OBJECT, costs).await;
    }
    info!("test all non case success!");

    true
}

pub async fn test2() -> bool {
    true
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

async fn clear_all(dec_id: &ObjectId, zone: &SimZone) {
    let stack = zone.get_shared_stack("zone1_device1");

    let device1 = stack.local_device_id();
    let device2 = zone.get_shared_stack("zone1_device2").local_device_id();
    let ood = zone.get_shared_stack("zone1_ood").local_device_id();

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

async fn open_access(stack: &SharedCyfsStack, _dec_id: &ObjectId, req_path: impl Into<String>, _perm: AccessPermissions) {
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
async fn test_outer_put_dec(_dec_id: &ObjectId, zone: &SimZone) -> u64 {

    info!("begin test_outer_put_dec...");
    let begin = std::time::Instant::now();

    //let dec_id = zone.get_shared_stack("zone1_device2").dec_id().unwrap().to_owned();
    let (_q, a) = qa_pair();
    let object_id = a.text_id().object_id().to_owned();

    let stack = zone.get_shared_stack("zone1_device1");
    let target_stack = zone.get_shared_stack("zone2_device2");

    let mut req =
        NONPutObjectOutputRequest::new_router(None, object_id.clone(), a.to_vec().unwrap());
    req.common.dec_id = Some(zone.get_shared_stack("zone1_device1").dec_id().unwrap().to_owned());
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

    let target_dec_id = zone.get_shared_stack("zone1_device1").dec_id().unwrap().clone();
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
    req.common.dec_id = Some(zone.get_shared_stack("zone1_device1").dec_id().unwrap().to_owned());
    req.common.target = Some(stack.local_device_id().into());

    let new_dec = new_dec("cross_dec", zone);
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
    open_access(&stack, &zone.get_shared_stack("zone1_device1").dec_id().unwrap().to_owned(), "/root/shared", AccessPermissions::Full).await;
    // 挂在树上
    let stub = stack.root_state_stub(Some(stack.local_device_id().object_id().to_owned()), None);
    let op_env = stub.create_path_op_env().await.unwrap();
    op_env.set_with_path("/root/shared", &object_id, None, true).await.unwrap();
    let _root_info = op_env.commit().await.unwrap();

    req.common.req_path = Some(req_path.format_string());
    req.common.dec_id = Some(zone.get_shared_stack("zone1_device1").dec_id().unwrap().to_owned());
    req.common.target = Some(stack.local_device_id().into());
    let ret = target_stack.non_service().get_object(req.clone()).await;
    let resp = ret.unwrap();
    let t = Text::clone_from_slice(&resp.object.object_raw).unwrap();
    assert_eq!(*t.text_id().object_id(), *a.text_id().object_id());
    assert_eq!(resp.object.object_id, *a.text_id().object_id());

    info!("cross zone get object success");
    let dur = begin.elapsed();
    info!("end test test_outer_put_dec: {:?}", dur);

    let costs = begin.elapsed().as_millis() as u64;

    costs

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
) -> u64 {
    info!("begin test_delete_object...");
    let begin = std::time::Instant::now();

    let mut req = NONDeleteObjectOutputRequest::new_router(
        Some(target.object_id().to_owned()),
        object_id.to_owned(),
        None,
    );
    req.common.dec_id = Some(dec_id.to_owned());

    req.common.target = Some(target.object_id().to_owned());
    let _resp = stack.non_service().delete_object(req).await.unwrap();
    info!("delete object success! {}", object_id);
    let dur = begin.elapsed();
    info!("end test_delete_object: {:?}", dur);

    let costs = begin.elapsed().as_millis() as u64;

    costs
}

async fn test_put_object(dec_id: &ObjectId, stack: &SharedCyfsStack) -> u64 {
    info!("begin test_put_object...");
    let begin = std::time::Instant::now();

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

    let dur = begin.elapsed();
    info!("end test_put_object: {:?}", dur);
    let costs = begin.elapsed().as_millis() as u64;

    costs

}

async fn test_get_object(dec_id: &ObjectId, stack: &SharedCyfsStack, target: &DeviceId, zone: &SimZone) -> u64 {
    info!("begin test_get_object...");
    let begin = std::time::Instant::now();
    let get_object_path = "/.cyfs/api/handler/pre_router/get_object/";
    // req_path 统一格式
    let req_path = RequestGlobalStatePath {
        global_state_category: None,
        global_state_root: None,
        dec_id: Some(dec_id.clone()),
        req_path: Some(get_object_path.to_owned()),
    };

    let req_path = req_path.format_string();
    // req_path层的权限
    let target_stack = zone.get_shared_stack("zone1_ood");
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
    let dur = begin.elapsed();
    info!("end test test_get_object: {:?}", dur);

    let costs = begin.elapsed().as_millis() as u64;

    costs

}