use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;
use zone_simulator::*;

pub async fn test() {
    zone_same_dec_call().await;
    zone_diff_dec_call().await;
    inter_zone_same_dec_call().await;

    info!("all call test case success!");
}

fn new_object(dec_id: &ObjectId, owner: Option<ObjectId>, id: &str) -> Text {
    let mut builder = Text::build(id, "test_crypto", "hello!")
        .no_create_time()
        .dec_id(dec_id.to_owned());
    if let Some(owner) = owner {
        builder = builder.owner(owner);
    }
    builder.build()
}

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage test_crypto dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

struct OnPostObjectHandler {
    panic: bool,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for OnPostObjectHandler
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        info!("recv post request: {}", param.request.object.object_id);

        if self.panic {
            panic!("should not reach here!!!!");
        }
        let resp = RouterHandlerPostObjectResult {
            action: RouterHandlerAction::Response,
            request: None,
            response: Some(Ok(NONPostObjectInputResponse {
                object: None,
            })),
        };

        Ok(resp)
    }
}

async fn zone_same_dec_call() {
    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2 = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

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
    let call_path = "/test/zone/same_dec/call";

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

    // attach post_object handler will be ok!
    let req_path = RequestGlobalStatePath::new(Some(dec1.clone()), Some(call_path.to_owned()));
    device2.router_handlers().post_object().add_handler(
        RouterHandlerChain::Handler,
        "zone_same_dec_call_handler_same_dec_ok",
        0,
        None,
        Some(req_path.to_string()),
        RouterHandlerAction::Default,
        Some(Box::new(OnPostObjectHandler { panic: false })),
    ).await.unwrap();

    // use other dec3 attach dec2's post_object, will failed!
    let dec3 = new_dec("zone_same_dec_call3");
    let device3 = device2
        .fork_with_new_dec(Some(dec3.clone()))
        .await
        .unwrap();
    device3.wait_online(None).await.unwrap();
    device3.router_handlers().post_object().add_handler(
        RouterHandlerChain::Handler,
        "zone_same_dec_call_handler_diff_dec_error",
        0,
        None,
        Some(req_path.to_string()),
        RouterHandlerAction::Default,
        Some(Box::new(OnPostObjectHandler { panic: true })),
    ).await.unwrap();


    // create object
    let object = new_object(dec1, None, "test_post");
    let object_raw = object.to_vec().unwrap();
    let object_id = object.desc().object_id();

    // post_object (device1, dec1) -> (decvice2, dec1)
    let mut req =
        NONPostObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, object_raw);
    let req_path = RequestGlobalStatePath::new(Some(dec1.clone()), Some(call_path.to_owned()));
    req.common.req_path = Some(req_path.to_string());

    let ret = device1.non_service().post_object(req.clone()).await;
    assert!(ret.is_ok());

    // post_object (device2, dec3) -> (decvice2, dec1) will error
    let ret = device3.non_service().post_object(req.clone()).await;
    let err = ret.err().unwrap();
    assert_eq!(err.code(), BuckyErrorCode::PermissionDenied);
}

async fn zone_diff_dec_call() {
    let dec1 = new_dec("User1Device1.call");
    let dec2 = new_dec("User1Device2.call");

    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1)
        .fork_with_new_dec(Some(dec1.clone()))
        .await
        .unwrap();
    device1.wait_online(None).await.unwrap();

    let device2 = TestLoader::get_shared_stack(DeviceIndex::User1Device2)
        .fork_with_new_dec(Some(dec2.clone()))
        .await
        .unwrap();
    device2.wait_online(None).await.unwrap();

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

    let call_path = "/test/zone/diff_dec/call";

    let object = new_object(&dec1, None, "test_post");
    let object_raw = object.to_vec().unwrap();
    let object_id = object.desc().object_id();

    let mut req =
        NONPostObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, object_raw);
    let req_path = RequestGlobalStatePath::new(Some(dec2.clone()), Some(call_path.to_owned()));
    req.common.req_path = Some(req_path.to_string());

    let ret = device1.non_service().post_object(req.clone()).await;
    assert!(ret.is_err());
    let err = ret.err().unwrap();
    assert_eq!(err.code(), BuckyErrorCode::PermissionDenied);

    // open_access
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Call);
    let item = GlobalStatePathAccessItem {
        path: call_path.to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };
    device2
        .root_state_meta_stub(None, None)
        .add_access(item)
        .await
        .unwrap();

    // post again
    let ret = device1.non_service().post_object(req.clone()).await;
    assert!(ret.is_err());

    let err = ret.err().unwrap();
    assert_eq!(err.code(), BuckyErrorCode::NotHandled);
}

async fn inter_zone_same_dec_call() {
    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

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
    let dec2 = device2.dec_id().unwrap();
    assert_eq!(dec1, dec2);
    let call_path = "/test/interzone/same_dec/call";

    let object = new_object(dec1, None, "test_post");
    let object_raw = object.to_vec().unwrap();
    let object_id = object.desc().object_id();

    let mut req = NONPostObjectOutputRequest::new_router(
        Some(device2.local_device_id().object_id().to_owned()),
        object_id,
        object_raw,
    );
    let req_path = RequestGlobalStatePath::new(Some(dec2.clone()), Some(call_path.to_owned()));
    req.common.req_path = Some(req_path.to_string());

    let ret = device1.non_service().post_object(req.clone()).await;
    assert!(ret.is_err());

    let err = ret.err().unwrap();
    assert_eq!(err.code(), BuckyErrorCode::PermissionDenied);

    // open_access
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Call);
    let item = GlobalStatePathAccessItem {
        path: call_path.to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };
    device2
        .root_state_meta_stub(None, None)
        .add_access(item)
        .await
        .unwrap();

    // try post again
    let ret = device1.non_service().post_object(req.clone()).await;
    assert!(ret.is_err());

    let err = ret.err().unwrap();
    assert_eq!(err.code(), BuckyErrorCode::NotHandled);
}
