use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use std::str::FromStr;
use zone_simulator::*;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage test root_state dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

pub async fn test() {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let device_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2_stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    test_gbk_path(&stack).await;

    test_router(&stack, &device_stack).await;
    test_cross_zone_router(&stack, &device2_stack).await;

    test_path_env(&stack).await;
    test_iterator(&stack).await;
}

pub async fn test_path_env(stack: &SharedCyfsStack) {
    // let dec_id = new_dec("root_state1");
    let root_state = stack.root_state_stub(None, None);
    let root_info = root_state.get_current_root().await.unwrap();
    info!("current root: {:?}", root_info);

    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x2_value = ObjectId::from_str("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();

    let op_env = root_state.create_path_op_env().await.unwrap();

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

pub async fn test_iterator(stack: &SharedCyfsStack) {
    let root_state = stack.root_state_stub(None, None);
    let root_info = root_state.get_current_root().await.unwrap();
    info!("current root: {:?}", root_info);

    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    // let x2_value = ObjectId::from_str("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();

    let op_env = root_state.create_path_op_env().await.unwrap();

    // 首先移除老的值，如果存在的话
    op_env.remove_with_path("/test/it", None).await.unwrap();

    let ret = op_env.get_by_path("/test/it").await.unwrap();
    assert!(ret.is_none());

    for i in 0..1000 {
        let key = format!("test_iterator_{:0>3}", i);
        op_env
            .insert_with_key("/test/it", &key, &x1_value)
            .await
            .unwrap();
    }

    op_env.commit().await.unwrap();

    // 测试枚举
    let single_env = root_state.create_single_op_env().await.unwrap();
    single_env.load_by_path("/test/it").await.unwrap();

    loop {
        let ret = single_env.next(10).await.unwrap();
        if ret.len() == 0 {
            break;
        }

        info!("it got list: {:?}", ret);
    }
}

pub async fn test_router(ood: &SharedCyfsStack, device: &SharedCyfsStack) {
    let ood_id = ood.local_device_id();
    // let device_id = device.local_device_id();

    let ood_root_state = ood.root_state_stub(None, None);
    let ood_root_info = ood_root_state.get_current_root().await.unwrap();

    let root_state = device.root_state_stub(Some(ood_id.object_id().clone()), None);
    let root_info = root_state.get_current_root().await.unwrap();

    assert_eq!(ood_root_info, root_info);

    let test_obj = cyfs_core::Text::create("test", "test-root-state-router", "");
    let test_id = test_obj.desc().calculate_id();

    let path = "/test_router/a";
    let env = root_state.create_path_op_env().await.unwrap();
    match env.insert_with_path(path, &test_id).await {
        Ok(_) => {
            info!("insert_with_path success! {}={}", path, test_id);
        }
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::AlreadyExists);
        }
    }
    env.commit().await.unwrap();

    let env = root_state.create_path_op_env().await.unwrap();
    let value = env.get_by_path(path).await.unwrap();
    assert!(value.is_some());
    assert_eq!(value, Some(test_id));

    let env = ood_root_state.create_path_op_env().await.unwrap();
    let value = env.get_by_path(path).await.unwrap();
    assert!(value.is_some());
    assert_eq!(value, Some(test_id));
}

pub async fn test_cross_zone_router(ood: &SharedCyfsStack, device: &SharedCyfsStack) {
    let ood_id = ood.local_device_id();
    // let device_id = device.local_device_id();

    //let ood_root_state = ood.root_state_stub(None, None);
    //let ood_root_info = ood_root_state.get_current_root().await.unwrap();

    let root_state = device.root_state_stub(Some(ood_id.object_id().clone()), None);
    match root_state.get_current_root().await {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        }
        Ok(_) => {
            unreachable!();
        }
    }

    match root_state.create_path_op_env().await {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        }
        Ok(_) => {
            unreachable!();
        }
    }
}

pub async fn test_gbk_path(stack: &SharedCyfsStack) {
    // let dec_id = new_dec("root_state1");
    let root_state = stack.root_state_stub(None, None);
    let root_info = root_state.get_current_root().await.unwrap();
    info!("current root: {:?}", root_info);

    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x2_value = ObjectId::from_str("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();

    let op_env = root_state.create_path_op_env().await.unwrap();

    // 首先移除老的值，如果存在的话
    op_env.remove_with_path("/xxx/八八八", None).await.unwrap();

    let ret = op_env.get_by_path("/xxx/八八八").await.unwrap();
    assert_eq!(ret, None);
    let ret = op_env.get_by_path("/xxx/八八八/七七七").await.unwrap();
    assert_eq!(ret, None);

    op_env
        .insert_with_key("/xxx/八八八", "七七七", &x1_value)
        .await
        .unwrap();

    let ret = op_env.get_by_path("/xxx/八八八/七七七").await.unwrap();
    assert_eq!(ret, Some(x1_value));

    let ret = op_env
        .remove_with_path("/xxx/八八八/六六六", None)
        .await
        .unwrap();
    assert_eq!(ret, None);

    let root = op_env.commit().await.unwrap();
    info!("new dec root is: {:?}", root);

    {
        let op_env = root_state.create_path_op_env().await.unwrap();
        op_env.remove_with_path("/gbk_set", None).await.unwrap();

        let ret = op_env.insert("/gbk_set/一二三", &x2_value).await.unwrap();
        assert!(ret);

        let ret = op_env.contains("/gbk_set/一二三", &x1_value).await.unwrap();
        assert!(!ret);

        let ret = op_env.insert("/gbk_set/一二三", &x1_value).await.unwrap();
        assert!(ret);

        let ret = op_env.insert("/gbk_set/一二三", &x1_value).await.unwrap();
        assert!(!ret);

        let ret = op_env.remove("/gbk_set/一二三", &x1_value).await.unwrap();
        assert!(ret);

        let ret = op_env.insert("/gbk_set/一二三", &x1_value).await.unwrap();
        assert!(ret);

        let root = op_env.commit().await.unwrap();
        info!("new dec root is: {:?}", root);
    }

    info!("test root_state complete!");
}

/*
pub async fn test_rs_access(ood: &SharedCyfsStack, device: &SharedCyfsStack) {
    let ood_id = ood.local_device_id();
    // let device_id = device.local_device_id();

    let ood_root_state = ood.root_state_stub(None, None);
    let ood_access = ood.root_state_access();

    let ood_root_info = ood_root_state.get_current_root().await.unwrap();
}
*/
