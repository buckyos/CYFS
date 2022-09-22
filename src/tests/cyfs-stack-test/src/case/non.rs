use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
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

fn new_object(dec_id: &ObjectId, id: &str) -> Text {
    Text::build(id, "test_header", "hello!")
        .no_create_time()
        .dec_id(dec_id.to_owned())
        .build()
}

fn gen_text_object_list(dec_id: &ObjectId) -> Vec<(Text, ObjectId)> {
    let mut list = vec![];

    let object = new_object(dec_id, "first-text");
    let object_id = object.text_id().object_id().to_owned();
    list.push((object, object_id));

    let object = new_object(dec_id, "second-text");
    let object_id = object.text_id().object_id().to_owned();
    list.push((object, object_id));

    list
}

pub async fn test() {
    // zone_same_dec_without_req_path().await;
    zone_get_with_req_path().await;

    info!("test all non case success!");
}

async fn zone_same_dec_without_req_path() {
    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2 = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
    let ood1 = TestLoader::get_shared_stack(DeviceIndex::User1OOD);

    let dec1 = device1.dec_id().unwrap();
    let dec2 = device2.dec_id().unwrap();
    assert_eq!(dec1, dec2);

    let object = new_object(&dec1, "zone_same_dec_without_req_path");
    let object_id = object.desc().calculate_id();

    // first delete
    let del_req = NONDeleteObjectOutputRequest::new_noc(
        object_id.to_owned(),
        None,
    );

    let _resp = device1.non_service().delete_object(del_req.clone()).await.unwrap();
    let _resp = device2.non_service().delete_object(del_req.clone()).await.unwrap();
    info!("delete object success! {}", object_id);

    let mut req =
        NONPutObjectOutputRequest::new_router(None, object_id.clone(), object.to_vec().unwrap());

    let mut access = AccessString::new(0);
    access.set_group_permissions(AccessGroup::CurrentZone, AccessPermissions::Full);
    access.set_group_permissions(AccessGroup::CurrentDevice, AccessPermissions::Full);
    access.set_group_permissions(AccessGroup::OwnerDec, AccessPermissions::Full);
    req.access = Some(access);

    device1.non_service().put_object(req).await.unwrap();

    // test get by same dec
    let req = NONGetObjectRequest::new_router(None, object_id, None);
    let ret = device1.non_service().get_object(req.clone()).await.unwrap();

    // test get by other dec
    let new_dec = new_dec("zone_same_dec_without_req_path");
    let device3 = device2.fork_with_new_dec(Some(new_dec.clone())).await.unwrap();
    device3.wait_online(None).await.unwrap();
    let ret = device3.non_service().get_object(req.clone()).await;
    assert!(ret.is_err());
    let err = ret.err().unwrap();
    assert_eq!(err.code(), BuckyErrorCode::PermissionDenied);
}

async fn zone_get_with_req_path() {
    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2 = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
    let ood1 = TestLoader::get_shared_stack(DeviceIndex::User1OOD);

    let new_dec = new_dec("zone_get_with_req_path");
    let device3 = device2.fork_with_new_dec(Some(new_dec.clone())).await.unwrap();
    device3.wait_online(None).await.unwrap();


    let dec1 = device1.dec_id().unwrap();
    let dec2 = device2.dec_id().unwrap();
    assert_eq!(dec1, dec2);

    let object = new_object(&dec1, "zone_same_dec_with_req_path");
    let object_id = object.desc().calculate_id();

    // first delete from device1 and device2' local cache
    let del_req = NONDeleteObjectOutputRequest::new_noc(
        object_id.to_owned(),
        None,
    );

    let _resp = device1.non_service().delete_object(del_req.clone()).await.unwrap();
    let _resp = device2.non_service().delete_object(del_req.clone()).await.unwrap();
    info!("delete object success! {}", object_id);

    // put object to ood with defualt access
    let mut req =
        NONPutObjectOutputRequest::new_router(None, object_id.clone(), object.to_vec().unwrap());

    device1.non_service().put_object(req).await.unwrap();

    let path = "/test/non/zone_same_dec_with_req_path";

    // dec1 modify ood's state with object_id
    let stub = device1.root_state_stub(Some(ood1.local_device_id().object_id().to_owned()), None);
    let op_env = stub.create_path_op_env().await.unwrap();
    op_env.set_with_path(path, &object_id, None, true).await.unwrap();
    let root_info = op_env.commit().await.unwrap();

    // dec2(==dec1) get with req_path from ood， ok
    let req_path = RequestGlobalStatePath::new(Some(dec2.to_owned()), Some(path));
    let mut get_req = NONGetObjectRequest::new_router(None, object_id, None);
    get_req.common.req_path = Some(req_path.to_string());

    let ret = device2.non_service().get_object(get_req.clone()).await;
    assert!(ret.is_ok());
    
    // must delete from device2(==device3)'s local cache
    let _resp = device2.non_service().delete_object(del_req.clone()).await.unwrap();

    // dec3 get with req_path from ood, reject
    let ret = device3.non_service().get_object(get_req.clone()).await;
    assert!(ret.is_err());
    let err = ret.err().unwrap();
    assert_eq!(err.code(), BuckyErrorCode::PermissionDenied);


    // dec1 open path's access for other dec
    // open_access
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Read);
    let item = GlobalStatePathAccessItem {
        path: path.to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };
    device1
        .root_state_meta_stub(Some(ood1.local_device_id().object_id().to_owned()), None)
        .add_access(item)
        .await
        .unwrap();

    // dec3 get with req_path, ok
    let ret = device3.non_service().get_object(get_req.clone()).await;
    assert!(ret.is_ok());

    let _resp = device3.non_service().delete_object(del_req.clone()).await.unwrap();

    // dec3 get with req_path and dec_root, ok
    let mut req_path = RequestGlobalStatePath::new(Some(dec2.to_owned()), Some(path));
    req_path.set_root(root_info.root);
    get_req.common.req_path = Some(req_path.to_string());
    let ret = device3.non_service().get_object(get_req.clone()).await;
    assert!(ret.is_ok());

    let _resp = device3.non_service().delete_object(del_req.clone()).await.unwrap();

    let mut req_path = RequestGlobalStatePath::new(Some(dec2.to_owned()), Some(path));
    req_path.set_dec_root(root_info.dec_root);
    get_req.common.req_path = Some(req_path.to_string());
    let ret = device3.non_service().get_object(get_req.clone()).await;
    assert!(ret.is_ok());
}