use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

pub async fn test() {
    zone_same_dec_call().await;
    zone_diff_dec_call().await;
    inter_zone_same_dec_call().await;
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


async fn zone_same_dec_call() {
    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2 = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    device1.root_state_meta_stub(None, None).clear_access().await.unwrap();
    device2.root_state_meta_stub(None, None).clear_access().await.unwrap();

    let dec1 = device1.dec_id().unwrap();
    let dec2 = device2.dec_id().unwrap();
    assert_eq!(dec1, dec2);
    let call_path = "/test/zone/call";

    let object = new_object(dec1, None, "test_post");
    let object_raw = object.to_vec().unwrap();
    let object_id = object.desc().object_id();

    let mut req =
        NONPostObjectOutputRequest::new_non(Some(device2.local_device_id()), object_id, object_raw);
    let req_path = RequestGlobalStatePath::new(Some(dec2.clone()), Some(call_path.to_owned()));
    req.common.req_path = Some(req_path.to_string());

    let ret = device1.non_service().post_object(req.clone()).await;
    assert!(ret.is_err());

    let err = ret.err().unwrap();
    assert_eq!(err.code(), BuckyErrorCode::NotHandled);
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

    device1.root_state_meta_stub(None, None).clear_access().await.unwrap();
    device2.root_state_meta_stub(None, None).clear_access().await.unwrap();

    let call_path = "/test/zone/call";

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

    device1.root_state_meta_stub(None, None).clear_access().await.unwrap();
    device2.root_state_meta_stub(None, None).clear_access().await.unwrap();

    let dec1 = device1.dec_id().unwrap();
    let dec2 = device2.dec_id().unwrap();
    assert_eq!(dec1, dec2);
    let call_path = "/test/interzone/call";

    let object = new_object(dec1, None, "test_post");
    let object_raw = object.to_vec().unwrap();
    let object_id = object.desc().object_id();

    let mut req =
        NONPostObjectOutputRequest::new_router(Some(device2.local_device_id().object_id().to_owned()), object_id, object_raw);
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
