use async_trait::async_trait;
use cyfs_util::EventListenerAsyncRoutine;
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
            // TODO: support physical stack  ood/runtime
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

fn new_object(dec_id: &ObjectId, id: &str) -> Text {
    Text::build(id, "test_header", "hello!")
        .no_create_time()
        .dec_id(dec_id.to_owned())
        .build()
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


pub async fn test(zone: &SimZone) -> bool {
    let dec1 = new_dec("User1Device1.non", zone);
    let dec2 = new_dec("User1Device2.non", zone);
    let dec3 = new_dec("User2Ood.non", zone);
    let device1 = zone.get_shared_stack("zone1_device1")
        .fork_with_new_dec(Some(dec1.clone()))
        .await
        .unwrap();
    device1.wait_online(None).await.unwrap();

    let device2 = zone.get_shared_stack("zone1_device2")
        .fork_with_new_dec(Some(dec2.clone()))
        .await
        .unwrap();
    device2.wait_online(None).await.unwrap();

    let ood2 = zone.get_shared_stack("zone2_ood")
    .fork_with_new_dec(Some(dec3.clone()))
    .await
    .unwrap();
    ood2.wait_online(None).await.unwrap();

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

    let call_path = "/test/zone/diff_dec/non";

    let object = new_object(&dec1, "test_non3");
    let object_raw = object.to_vec().unwrap();
    let object_id = object.desc().object_id();

    // open_access
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Call);
    access.set_group_permission(AccessGroup::FriendZone, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Call);

    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
    access.set_group_permission(AccessGroup::FriendZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Read);

    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    access.set_group_permission(AccessGroup::FriendZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Write);
    
    let item = GlobalStatePathAccessItem {
        path: call_path.to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };
    device2
        .root_state_meta_stub(None, None)
        .add_access(item.clone())
        .await
        .unwrap();

    ood2
        .root_state_meta_stub(None, None)
        .add_access(item)
        .await
        .unwrap();

    /* 
        同zone 跨dec
    */
    // put_object_same_zone_diff_dec 不允许跨zone put, 允许跨zone get
    {
        info!("begin test_put_object...");
        let begin = std::time::Instant::now();

        let mut req =
        NONPutObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, object_raw);
        let req_path = RequestGlobalStatePath::new(Some(dec2.clone()), Some(call_path.to_owned()));
        req.common.req_path = Some(req_path.to_string());

        let ret = device1.non_service().put_object(req.clone()).await.unwrap();
        match ret.result {
            NONPutObjectResult::Accept => {
                info!("first put_object success! {}", object_id);
            }
            _ => {
                unreachable!();
            }
        }

        let dur = begin.elapsed();
        info!("end test_put_object: {:?}", dur);
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, NON_PUT_OBJECT, costs).await;
    }

    // get_object_same_zone_diff_dec 允许跨zone get
    {
        info!("begin test_get_object...");
        let begin = std::time::Instant::now();

        let req =
        NONGetObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, None);
        //let req_path = RequestGlobalStatePath::new(Some(dec2.clone()), Some(call_path.to_owned()));
        //req.common.req_path = Some(req_path.to_string());

        let ret = device1.non_service().get_object(req.clone()).await.unwrap();
        info!("test_get_object: {}", ret);

        let dur = begin.elapsed();
        info!("end test_get_object: {:?}", dur);
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, NON_GET_OBJECT, costs).await;
    }
    // post_object_same_zone_diff_dec
    {
        info!("begin test_post_object...");
        let begin = std::time::Instant::now();
        let req_path = RequestGlobalStatePath::new(Some(dec2.clone()), Some(call_path.to_owned()));
        device2.router_handlers().post_object().add_handler(
            RouterHandlerChain::Handler,
            "post_object_same_zone_diff_dec",
            0,
            None,
            Some(req_path.to_string()),
            RouterHandlerAction::Default,
            Some(Box::new(OnPostObject { })),
        ).await.unwrap();
    
        // post_object (device1, dec1) -> (decvice2, dec2) 
        let (q, a) = qa_pair();
        let object_id = q.text_id().object_id().to_owned();
    
        let mut req = NONPostObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, q.to_vec().unwrap());
        req.common.dec_id = Some(dec2.clone());
    
    
        let req_path = RequestGlobalStatePath::new(Some(dec2.clone()), Some(call_path.to_owned()));
        req.common.req_path = Some(req_path.to_string());
    
        let ret = device1.non_service().post_object(req.clone()).await;
        assert!(ret.is_ok());
        let resp = ret.unwrap();
    
        let t = Text::clone_from_slice(&resp.object.unwrap().object_raw).unwrap();
        assert_eq!(*t.text_id().object_id(), *a.text_id().object_id());

        let dur = begin.elapsed();
        info!("end test_post_object: {:?}", dur);
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, NON_POST_OBJECT, costs).await;
    }
    // delete_object_same_zone_diff_dec
    {
        info!("begin test_delete_object...");
        let begin = std::time::Instant::now();

        let req =
        NONDeleteObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, None);
        // 这里的路径之前put object的时候必须挂在树上 root_state
        //let req_path = RequestGlobalStatePath::new(Some(dec2.clone()), Some(call_path.to_owned()));
        //req.common.req_path = Some(req_path.to_string());

        let ret = device1.non_service().delete_object(req.clone()).await.unwrap();
        info!("test_delete_object: {}", ret);

        let dur = begin.elapsed();
        info!("end test_delete_object: {:?}", dur);
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, NON_DELETE_OBJECT, costs).await;
    }

    /* 
        跨zone 跨dec  get/post object
    */
    {
        // 生成临时object数据
        let object = new_object(&dec1, "test_outer_non6");
        let object_raw = object.to_vec().unwrap();
        let object_id = object.desc().object_id();
        let mut req =
        NONPutObjectOutputRequest::new_router(Some(ood2.local_device_id().object_id().to_owned()), object_id, object_raw);
        let req_path = RequestGlobalStatePath::new(Some(dec3.clone()), Some(call_path.to_owned()));
        req.common.req_path = Some(req_path.to_string());
        // 权限位操作
        let mut access = AccessString::default();
        access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Read);
        access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Write);
        access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Call);

        access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Read);
        access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Write);
        access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Call);

        // 这里是object层对象
        req.access = Some(access);

        let ret = ood2.non_service().put_object(req.clone()).await.unwrap();
        match ret.result {
            NONPutObjectResult::Accept => {
                info!("temp put_object success! {}", object_id);
            }
            _ => {
                unreachable!();
            }
        }
        // 跨zone跨dec get
        {
            info!("begin test_outer_get_object...");
            let begin = std::time::Instant::now();
    
            let req =
            NONGetObjectOutputRequest::new_router(Some(ood2.local_device_id().object_id().to_owned()), object_id, None);

            let ret = device1.non_service().get_object(req.clone()).await.unwrap();
            info!("test_outer_get_object: {}", ret);
    
            let dur = begin.elapsed();
            info!("end test_outer_get_object: {:?}", dur);
            let costs = begin.elapsed().as_millis() as u64;
            Stat::write(zone, NON_OUTER_GET_OBJECT, costs).await;
        }
        // 跨zone跨dec post
        {
            info!("begin test_outer_post_object...");
            let begin = std::time::Instant::now();
            let req_path = RequestGlobalStatePath::new(Some(dec3.clone()), Some(call_path.to_owned()));
            ood2.router_handlers().post_object().add_handler(
                RouterHandlerChain::Handler,
                "post_object_outer_zone_diff_dec",
                0,
                None,
                Some(req_path.to_string()),
                RouterHandlerAction::Default,
                Some(Box::new(OnPostObject { })),
            ).await.unwrap();
        
            // post_object (device1, dec1) -> (ood2, dec3) 
            let (q, a) = qa_pair();
            let object_id = q.text_id().object_id().to_owned();
        
            let mut req = NONPostObjectOutputRequest::new_router(Some(ood2.local_device_id().object_id().to_owned()), object_id, q.to_vec().unwrap());
            req.common.dec_id = Some(dec3.clone());
        
        
            let req_path = RequestGlobalStatePath::new(Some(dec3.clone()), Some(call_path.to_owned()));
            req.common.req_path = Some(req_path.to_string());
        
            let ret = device1.non_service().post_object(req.clone()).await;
            assert!(ret.is_ok());
            let resp = ret.unwrap();
        
            let t = Text::clone_from_slice(&resp.object.unwrap().object_raw).unwrap();
            assert_eq!(*t.text_id().object_id(), *a.text_id().object_id());
    
            let dur = begin.elapsed();
            info!("end test_outer_post_object: {:?}", dur);
            let costs = begin.elapsed().as_millis() as u64;
            Stat::write(zone, NON_OUTER_POST_OBJECT, costs).await;
        }

        // 销毁临时object数据
        let req =
        NONDeleteObjectOutputRequest::new_router(Some(ood2.local_device_id().object_id().to_owned()), object_id, None);
        let _ret = ood2.non_service().delete_object(req.clone()).await.unwrap();
    }

    info!("test all non case success!");

    true
}

pub async fn test2() -> bool {
    true
}