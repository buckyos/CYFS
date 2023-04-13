use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

use std::str::FromStr;

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

    test_isolate_path_env_leak(&stack).await;
    warn!("test isolate path env leak success!");
    // async_std::task::sleep(std::time::Duration::from_secs(1000)).await;

    test_group_state().await;
    test_load_with_cache(&device_stack).await;

    test_storage(&device_stack).await;

    test_gbk_path(&stack).await;

    test_router(&stack, &device_stack).await;
    test_cross_zone_router(&stack, &device2_stack).await;

    test_path_env(&stack).await;
    test_path_env_update(&stack).await;
    test_isolate_path_env(&stack).await;
    test_iterator(&stack).await;

    info!("test root state all cases success!");
}

async fn test_group_state() {
    let stack = TestLoader::get_stack(DeviceIndex::User1OOD);

    let isolate_id = new_dec("test-group");
    let group_state = stack
        .global_state_manager()
        .get_global_state(GlobalStateCategory::RootState, &isolate_id)
        .await;
    let group_state = if group_state.is_none() {
        let _ret = stack
            .global_state_manager()
            .load_global_state(GlobalStateCategory::RootState, &isolate_id, None, false)
            .await
            .unwrap();

        let ret = stack
            .global_state_manager()
            .load_root_state(&isolate_id, None, true)
            .await
            .unwrap();
        ret.unwrap()
    } else {
        group_state.unwrap().clone_processor()
    };

    let _ = group_state.get_dec_root(&isolate_id);
    let dec_manager = group_state
        .get_dec_root_manager(&isolate_id, true)
        .await
        .unwrap();
    let op_env = dec_manager.create_op_env(None).unwrap();
    op_env
        .set_with_path("/a/b", &isolate_id, &None, true)
        .await
        .unwrap();
    let dec_root = op_env.commit().await.unwrap();

    assert_eq!(dec_root, dec_manager.get_current_root());

    info!("test group state complete!");
}

async fn test_load_with_cache(stack: &SharedCyfsStack) {
    let root_state = stack.root_state_stub(None, None);

    let root_info = root_state.get_current_root().await.unwrap();
    info!("current root: {:?}", root_info);

    for _ in 0..5 {
        let op_env = root_state.create_path_op_env().await.unwrap();

        let ret = op_env.get_by_path("/dsg-service/contracts/").await.unwrap();
        assert_eq!(ret, None);
    }

    for _ in 0..5 {
        let op_env = root_state.create_single_op_env().await.unwrap();

        let ret = op_env.load_by_path("/dsg-service/contracts/").await;
        assert!(ret.is_err());
    }
}

pub async fn test_path_env_update(stack: &SharedCyfsStack) {
    // let dec_id = new_dec("root_state1");
    let root_state = stack.root_state_stub(None, None);
    let root_info = root_state.get_current_root().await.unwrap();
    info!("current root: {:?}", root_info);

    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x2_value = ObjectId::from_str("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();

    let path = "/test/update";

    let op_env = root_state.create_path_op_env().await.unwrap();

    // 首先移除老的值，如果存在的话
    op_env.remove_with_path(path, None).await.unwrap();

    let ret = op_env.get_by_path(path).await.unwrap();
    assert_eq!(ret, None);

    op_env.insert_with_path(path, &x1_value).await.unwrap();

    let root = op_env.update().await.unwrap();

    {
        let op_env = root_state.create_path_op_env().await.unwrap();
        let ret = op_env.get_by_path(path).await.unwrap();
        assert_eq!(ret, Some(x1_value));
    }

    let ret = op_env.get_by_path(path).await.unwrap();
    assert_eq!(ret, Some(x1_value));

    let ret = op_env
        .set_with_path(path, &x2_value, Some(x1_value.clone()), false)
        .await
        .unwrap();
    assert_eq!(ret, Some(x1_value));

    let root2 = op_env.update().await.unwrap();
    assert_ne!(root, root2);

    let root3 = op_env.commit().await.unwrap();
    assert_eq!(root3, root2);

    {
        let op_env = root_state.create_path_op_env().await.unwrap();
        let ret = op_env.get_by_path(path).await.unwrap();
        assert_eq!(ret, Some(x2_value));
    }
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

    let mut all_list = vec![];
    loop {
        let mut ret = single_env.next(10).await.unwrap();
        if ret.len() == 0 {
            break;
        }

        info!("it got list: {:?}", ret);
        all_list.append(&mut ret);
    }

    single_env.reset().await.unwrap();
    let mut all_list2 = vec![];
    loop {
        let mut ret = single_env.next(10).await.unwrap();
        if ret.len() == 0 {
            break;
        }

        info!("it got list: {:?}", ret);
        all_list2.append(&mut ret);
    }

    assert_eq!(all_list, all_list2);

    let all_list3 = single_env.list().await.unwrap();
    assert_eq!(all_list, all_list3);
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

        let list = op_env.list("/gbk_set").await.unwrap();
        assert_eq!(list.len(), 1);
        if let ObjectMapContentItem::Map((k, _v)) = &list[0] {
            assert_eq!(k, "一二三");
        }

        info!("list: {:?}", list);

        let list = op_env.list("/gbk_set/一二三").await.unwrap();
        assert_eq!(list.len(), 2);

        for item in list {
            if let ObjectMapContentItem::Set(v) = item {
                assert!(v == x1_value || v == x2_value);
            } else {
                unreachable!();
            }
        }

        let root = op_env.commit().await.unwrap();
        info!("new dec root is: {:?}", root);
    }

    info!("test root_state gbk complete!");
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

pub async fn test_storage(s: &SharedCyfsStack) {
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x2_value = ObjectId::from_str("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();

    let s = s
        .fork_with_new_dec(Some(cyfs_core::get_system_dec_app().to_owned()))
        .await
        .unwrap();
    s.wait_online(None).await.unwrap();

    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::RootState,
            "/",
            ObjectMapSimpleContentType::Map,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();

        let map = StateStorageMap::new(storage);

        let list = map.list().await.unwrap();
        info!("list: {:?}", list);

        map.save().await.unwrap();
    }

    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::RootState,
            "/user/friends",
            ObjectMapSimpleContentType::Map,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();

        let map = StateStorageMap::new(storage);
        match map.remove("user1").await.unwrap() {
            Some(value) => {
                info!("remove current value: {}", value);
            }
            None => {
                info!("current value is none!");
            }
        }

        let list = map.list().await.unwrap();
        assert!(list.is_empty());

        map.save().await.unwrap();
    }

    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::RootState,
            "/user/friends",
            ObjectMapSimpleContentType::Map,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();

        let map = StateStorageMap::new(storage);
        let v = map.get("user1").await.unwrap();
        assert!(v.is_none());

        let prev = map.set("user1", &x1_value).await.unwrap();
        assert!(prev.is_none());

        map.storage().save().await.unwrap();

        let prev = map.set("user1", &x2_value).await.unwrap();
        assert_eq!(prev, Some(x1_value));

        map.storage().save().await.unwrap();
        map.storage().save().await.unwrap();

        let list = map.list().await.unwrap();
        assert!(list.len() == 1);
        let item = &list[0];
        assert_eq!(item.0, "user1");
        assert_eq!(item.1, x2_value);

        map.into_storage().abort().await;
    }

    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::RootState,
            "/user/friends",
            ObjectMapSimpleContentType::Map,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();

        let map = StateStorageMap::new(storage);
        let v = map.get("user1").await.unwrap();
        assert_eq!(v, Some(x2_value));

        map.abort().await;
    }

    // test auto_save
    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::LocalCache,
            "/user/friends",
            ObjectMapSimpleContentType::Map,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();
        storage.start_save(std::time::Duration::from_secs(5));

        let map = StateStorageMap::new(storage);
        map.remove("user2").await.unwrap();
        map.set("user2", &x1_value).await.unwrap();

        info!("will wait for auto save for user2...");
        async_std::task::sleep(std::time::Duration::from_secs(10)).await;

        info!("will drop map for user2...");
        drop(map);

        {
            let storage = s.global_state_storage_ex(
                GlobalStateCategory::LocalCache,
                "/user/friends",
                ObjectMapSimpleContentType::Map,
                None,
                Some(cyfs_core::get_system_dec_app().to_owned()),
            );

            storage.init().await.unwrap();

            let map = StateStorageMap::new(storage);
            let ret = map.get("user2").await.unwrap();
            assert_eq!(ret, Some(x1_value));
        }
    }

    // test auto_save and drop
    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::LocalCache,
            "/user/friends",
            ObjectMapSimpleContentType::Map,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();

        let map = StateStorageMap::new(storage);
        map.remove("user2").await.unwrap();
        map.set("user2", &x1_value).await.unwrap();
        assert!(map.storage().is_dirty());

        map.storage().start_save(std::time::Duration::from_secs(5));
        async_std::task::sleep(std::time::Duration::from_secs(5)).await;
    }

    // test some set cases
    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::RootState,
            "/user/index",
            ObjectMapSimpleContentType::Set,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();

        let set = StateStorageSet::new(storage);
        set.remove(&x1_value).await.unwrap();
        set.remove(&x2_value).await.unwrap();

        set.save().await.unwrap();
        set.abort().await;
    }

    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::RootState,
            "/user/index",
            ObjectMapSimpleContentType::Set,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();

        let set = StateStorageSet::new(storage);
        assert!(!set.contains(&x1_value).await.unwrap());
        assert!(!set.contains(&x2_value).await.unwrap());

        set.insert(&x1_value).await.unwrap();
        assert!(set.contains(&x1_value).await.unwrap());

        set.save().await.unwrap();
        let ret = set.insert(&x2_value).await.unwrap();
        assert!(ret);

        let ret = set.insert(&x2_value).await.unwrap();
        assert!(!ret);

        set.save().await.unwrap();
    }

    {
        let storage = s.global_state_storage_ex(
            GlobalStateCategory::RootState,
            "/user/index",
            ObjectMapSimpleContentType::Set,
            None,
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        storage.init().await.unwrap();

        let set = StateStorageSet::new(storage);

        let list = set.list().await.unwrap();
        assert!(list.len() == 2);
        assert!(list.iter().find(|&&v| v == x1_value).is_some());
        assert!(list.iter().find(|&&v| v == x2_value).is_some());

        set.abort().await;
    }

    info!("state storage test complete!");
}

async fn test_isolate_path_env(stack: &SharedCyfsStack) {
    let root_state = stack.root_state_stub(None, None);

    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // create sub tree
    let path_env = root_state.create_isolate_path_op_env().await.unwrap();
    path_env.get_by_path("/a/b").await.unwrap_err();

    path_env
        .create_new(ObjectMapSimpleContentType::Map)
        .await
        .unwrap();

    path_env.insert_with_path("/a/b", &x1_value).await.unwrap();
    let ret = path_env
        .set_with_path("/a/b", &x1_value, None, false)
        .await
        .unwrap();
    assert_eq!(ret, Some(x1_value));
    let ret = path_env
        .set_with_path("/a/b", &x1_value2, Some(x1_value), false)
        .await
        .unwrap();
    assert_eq!(ret, Some(x1_value));
    let ret = path_env.get_by_path("/a/b").await.unwrap();
    assert_eq!(ret, Some(x1_value2));
    let ret = path_env.get_by_path("/a/x").await.unwrap();
    assert_eq!(ret, None);

    path_env.insert_with_path("/a/c", &x1_value).await.unwrap();

    path_env
        .create_new_with_path("/s", ObjectMapSimpleContentType::Set)
        .await
        .unwrap();
    path_env
        .create_new_with_path("/s2", ObjectMapSimpleContentType::Set)
        .await
        .unwrap();
    path_env.insert("/s", &x1_value).await.unwrap();
    path_env.insert("/s", &x1_value2).await.unwrap();

    let ret = path_env.metadata("/s2").await.unwrap();
    assert_eq!(ret.count, 0);
    assert_eq!(ret.content_type, ObjectMapSimpleContentType::Set);

    let info = path_env.get_current_root().await.unwrap();
    let info2 = path_env.commit().await.unwrap();

    assert_eq!(info, info2);

    // attach to root-state and check with full path of root-state
    let op_env = root_state.create_path_op_env().await.unwrap();
    let ret = op_env.remove_with_path("/i", None).await.unwrap();
    if ret.is_some() {
        // WARN sub tree always the same! if sub tree changed in future, this assert will triggered once!
        // assert_eq!(ret, Some(info.root));
    }

    op_env.insert_with_path("/i", &info.root).await.unwrap();

    let value = op_env.get_by_path("/i/a/b").await.unwrap();
    assert_eq!(value, Some(x1_value2));

    let value = op_env.get_by_path("/i/a/c").await.unwrap();
    assert_eq!(value, Some(x1_value));

    let value = op_env.get_by_path("/i").await.unwrap();
    assert_eq!(value, Some(info.root));

    let value = op_env.get_by_path("/i/a/x").await.unwrap();
    assert_eq!(value, None);

    let ret = op_env.contains("/i/s", &x1_value).await.unwrap();
    assert!(ret);

    let ret = op_env.contains("/i/s", &x1_value2).await.unwrap();
    assert!(ret);

    let ret = op_env.contains("/i/s", &info.root).await.unwrap();
    assert!(!ret);

    let ret = op_env.metadata("/i/s2").await.unwrap();
    assert_eq!(ret.count, 0);
    assert_eq!(ret.content_type, ObjectMapSimpleContentType::Set);

    op_env.commit().await.unwrap();

    info!("test_isolate_path_env complete!");
}

fn random_object(i: usize, j: usize) -> ObjectId {
    let id = format!("random_object_{}_{}", i, j);
    Text::create(&id, id.clone(), id.clone())
        .text_id()
        .object_id()
        .clone()
}

async fn test_isolate_path_env_leak(stack: &SharedCyfsStack) {
    let root_state = stack.root_state_stub(None, None);

    let mut tasks = vec![];
    for i in 0..100 {
        let root_state = root_state.clone();
        let t = async_std::task::spawn(async move {
            warn!("will run isolate task: index={}", i);

            let path_env = root_state.create_isolate_path_op_env().await.unwrap();
            path_env
                .create_new(ObjectMapSimpleContentType::Map)
                .await
                .unwrap();

            for j in 0..1000 {
                let object_id = random_object(i, j);
                let path = format!("/{}", object_id.to_string());
                path_env.insert_with_path(&path, &object_id).await.unwrap();
            }

            warn!("test isolate path will commit, index={}", i);
            path_env.commit().await.unwrap();
            warn!("test isolate path commit complete, index={}", i);
        });

        tasks.push(t);
    }

    futures::future::join_all(tasks).await;
}
