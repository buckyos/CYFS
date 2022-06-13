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

pub async fn test() {
    let dec_id = new_dec("test-non");

    clear_all(&dec_id).await;

    async_std::task::spawn(async move {
        loop {
            let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
            test_put_object(&dec_id, &stack).await;

            async_std::task::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    test_outer_put(&dec_id).await;

    let target;
    {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
        // stack.wait_online(None).await.unwrap();
        target = stack.local_device_id();
        test_put_object(&dec_id, &stack).await;
    }

    let list = {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
        test_select(&dec_id, &stack, &target).await;
        test_select_with_req_path(&dec_id, &stack, &target).await
    };

    for object_id in list {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
        test_delete_object(&object_id, &dec_id, &stack, &target).await;
    }

    info!("test all non case success!");
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

async fn test_select(dec_id: &ObjectId, stack: &SharedCyfsStack, target: &DeviceId) {
    let mut filter = SelectFilter::default();
    filter.obj_type = Some(CoreObjectType::Text.into());
    filter.dec_id = Some(dec_id.clone());

    let mut req = NONSelectObjectRequest::new(NONAPILevel::NON, filter, None);
    req.common.target = Some(target.object_id().to_owned());
    let resp = stack.non_service().select_object(req).await.unwrap();

    /*
    // used for clear old data
    for item in &resp.objects {
        test_delete_object(&item.object.as_ref().unwrap().object_id, dec_id, stack, target).await;
    }
    */
    
    assert_eq!(resp.objects.len(), 2);
}

async fn test_select_with_req_path(
    dec_id: &ObjectId,
    stack: &SharedCyfsStack,
    target: &DeviceId,
) -> Vec<ObjectId> {
    let mut filter = SelectFilter::default();
    filter.obj_type = Some(CoreObjectType::Text.into());
    filter.dec_id = Some(dec_id.clone());

    let mut req = NONSelectObjectRequest::new(NONAPILevel::NON, filter, None);
    req.common.target = Some(target.object_id().to_owned());
    req.common.req_path = Some("/test/select".to_owned());
    req.common.dec_id = Some(dec_id.to_owned());

    let resp = stack.non_service().select_object(req).await.unwrap();
    assert_eq!(resp.objects.len(), 2);

    resp.objects
        .into_iter()
        .map(|item| item.object.unwrap().object_id)
        .collect()
}
