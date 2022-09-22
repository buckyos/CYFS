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
    zone_same_dec_without_req_path().await;

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

async fn test_outer_put(dec_id: &ObjectId) {
    let object = new_object(dec_id, "first-outter-text");
    let object_id = object.text_id().object_id().to_owned();

    let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device2);
    let target_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);

    let mut req =
        NONPutObjectOutputRequest::new_router(None, object_id.clone(), object.to_vec().unwrap());
    req.common.dec_id = Some(dec_id.clone());
    req.common.target = Some(target_stack.local_device_id().into());

    let ret = stack.non_service().put_object(req).await;
    match ret {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        }
        Ok(_) => unreachable!(),
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

        info!("will test put object to ood: {}", object_id);

        let mut req = NONPutObjectOutputRequest::new_router(
            None,
            object_id.clone(),
            object.to_vec().unwrap(),
        );
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
            _ => {
                unreachable!();
            }
        }
    }
}
