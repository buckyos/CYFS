use async_trait::async_trait;
use crate::{Bench, BenchEnv, sim_zone::SimZone, Stat};
use log::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use super::constant::*;

pub struct RootStateBench {}

#[async_trait]
impl Bench for RootStateBench {
    async fn bench(&self, env: BenchEnv, zone: &SimZone, _ood_path: String, t: u64) -> bool {
        info!("begin test GlobalStateBench...");
        let begin = std::time::Instant::now();
        let ret = if env == BenchEnv::Simulator {
            for _ in 0..t {
                let _ret = test(zone).await;
            }
            true
        } else {
            // TODO: support physical stack  ood/runtime
            true
        };

        let dur = begin.elapsed();
        info!("end test GlobalStateBench: {:?}", dur);
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, GLOABL_STATE_ALL_IN_ONE, costs).await;

        ret

    }

    fn name(&self) -> &str {
        "Root State Bench"
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

pub async fn test(zone: &SimZone) {
    let dec1 = new_dec("User1Device1.rootstate", zone);
    let dec2 = new_dec("User1Device2.rootstate", zone);
    let dec3 = new_dec("User2Ood.rootstate", zone);
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

    let call_path = "/root/shared";
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Read);

    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OthersZone, AccessPermission::Write);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Write);

    let item = GlobalStatePathAccessItem {
        path: call_path.to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };
    device1
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
        测试root-state的同zone的跨dec操作 需要配合权限
    */
    {
        info!("begin test PathOpEnv diff dec...");
        let begin_root = std::time::Instant::now();

        let x1_value = ObjectId::from_base58("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
        let x2_value = ObjectId::from_base58("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();
    
        let root_state = device1.root_state_stub(None, None);
        let root_info = root_state.get_current_root().await.unwrap();
        debug!("current root: {:?}", root_info);
    
        // 目标req_path层, dec-id开启对应的权限才可以操作
        open_access(&device1, &dec1, call_path, AccessPermissions::None).await;
    
        let access = RootStateOpEnvAccess::new("/", AccessPermissions::Full);   // 对跨dec路径操作这个perm才work
        let op_env = root_state.create_path_op_env_with_access(Some(access)).await.unwrap();
    
        // test create_new
        op_env.remove_with_path("/root/shared/new", None).await.unwrap();
        
        let begin = std::time::Instant::now();
        op_env
            .create_new_with_path("/root/shared/new/a", ObjectMapSimpleContentType::Map)
            .await
            .unwrap();
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, ROOT_STATE_CREATE_NEW_OPERATION, costs).await;
        let begin = std::time::Instant::now();
        op_env
            .create_new_with_path("/root/shared/new/c", ObjectMapSimpleContentType::Set)
            .await
            .unwrap();
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, ROOT_STATE_CREATE_NEW_OPERATION, costs).await;
        if let Err(e) = op_env
            .create_new_with_path("/root/shared/new/a", ObjectMapSimpleContentType::Map)
            .await
        {
            assert!(e.code() == BuckyErrorCode::AlreadyExists);
        } else {
            unreachable!();
        }
    
        if let Err(e) = op_env
            .create_new_with_path("/root/shared/new/c", ObjectMapSimpleContentType::Map)
            .await
        {
            assert!(e.code() == BuckyErrorCode::AlreadyExists);
        } else {
            unreachable!();
        }
    
        let begin = std::time::Instant::now();
        // 首先移除老的值，如果存在的话
        op_env.remove_with_path("/root/shared/x/b", None).await.unwrap();
        let costs = begin.elapsed().as_millis() as u64;
        // 记录下耗时到本地device
        Stat::write(zone, ROOT_STATE_REMOVE_OPERATION, costs).await;
    
        let ret = op_env.get_by_path("/root/shared/x/b").await.unwrap();
        assert_eq!(ret, None);
        let ret = op_env.get_by_path("/root/shared/x/b/c").await.unwrap();
        assert_eq!(ret, None);
    
        let begin = std::time::Instant::now();
        op_env
            .insert_with_key("/root/shared/x/b", "c", &x1_value)
            .await
            .unwrap();
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, ROOT_STATE_INSERT_OPERATION, costs).await;

        /* 
            root_state 跨zone access(get_object_by_key和list)
        */
        // {
        //     // 权限控制acl root_state meta => rmeta
        //     let meta = device1.root_state_meta_stub(None, None);
        //     // 为其他Zone的desc_id开放req_path的读写权限
        //     let item = GlobalStatePathAccessItem {
        //         path: call_path.into(),
        //         access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
        //             zone: None,
        //             zone_category: Some(DeviceZoneCategory::OtherZone),
        //             dec: Some(dec3.to_owned()),
        //             access: AccessPermissions::ReadAndWrite as u8,
        //         }),
        //     };

        //     meta.add_access(item).await.unwrap();

        //     assert_eq!(dec1.to_string(), "9tGpLNnA8xsobowPwB4WTb6j5D1w8h9k48F7NsrX9Vwi");

        //     //let root_state = ood2.root_state_stub(None, Some(dec1.to_owned()));
        //     // FIXME: direct operation get_by_path/list
        //     let root_state = ood2.root_state_stub(None, Some(dec3.to_owned()));

        //     let access = RootStateOpEnvAccess::new(call_path, AccessPermissions::ReadAndWrite); // 对跨dec路径操作这个perm才work
        //     let op_env = root_state.create_path_op_env_with_access(Some(access)).await.unwrap();
        //     let ret = op_env.get_by_path("/root/shared/x/b/c").await.unwrap();
        //     assert_eq!(ret, Some(x1_value));
        // }
    
        let begin = std::time::Instant::now();
        let ret = op_env.get_by_path("/root/shared/x/b/c").await.unwrap();
        assert_eq!(ret, Some(x1_value));
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, ROOT_STATE_GET_OPERATION, costs).await;
    
        let ret = op_env.remove_with_path("/root/shared/x/b/d", None).await.unwrap();
        assert_eq!(ret, None);
    
        let begin = std::time::Instant::now();
    
        let _root = op_env.commit().await.unwrap();
        let costs = begin.elapsed().as_millis() as u64;
        Stat::write(zone, ROOT_STATE_COMMIT_OPERATION, costs).await;

        info!("test op env path complete!");
    
        let dur = begin_root.elapsed();
        info!("end test PathOpEnv diff dec: {:?}", dur);
        let costs = begin_root.elapsed().as_millis() as u64;
        Stat::write(zone, ROOT_STATE_MAP, costs).await;

        {
            let begin = std::time::Instant::now();
            // create_path_op_env None access默认权限操作自己dec_id
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
    
            let _root = op_env.commit().await.unwrap();
    
            let costs = begin.elapsed().as_millis() as u64;
            Stat::write(zone, ROOT_STATE_SET, costs).await;
        }
    }

    /* 
        测试local-cache的同zone，覆盖map和set
    */
    {
        let x1_value = ObjectId::from_base58("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
        let x2_value = ObjectId::from_base58("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();
        {
            let begin = std::time::Instant::now();
            let local_cache = device1.local_cache_stub(None);
            let op_env = local_cache.create_path_op_env().await.unwrap();
            // test create_new
            op_env.remove_with_path("/root/shared/new", None).await.unwrap();
            
            if let Err(e) = op_env
                .create_new_with_path("/root/shared/new/a", ObjectMapSimpleContentType::Map)
                .await
            {
                assert!(e.code() == BuckyErrorCode::AlreadyExists);
            }

            if let Err(e) = op_env
                .create_new_with_path("/root/shared/new/c", ObjectMapSimpleContentType::Map)
                .await
            {
                assert!(e.code() == BuckyErrorCode::AlreadyExists);
            }

            // 首先移除老的值，如果存在的话
            op_env.remove_with_path("/root/shared/x/b", None).await.unwrap();
        
            let ret = op_env.get_by_path("/root/shared/x/b").await.unwrap();
            assert_eq!(ret, None);
            let ret = op_env.get_by_path("/root/shared/x/b/c").await.unwrap();
            assert_eq!(ret, None);
        
            op_env
                .insert_with_key("/root/shared/x/b", "c", &x1_value)
                .await
                .unwrap();
        
            let _ret = op_env.get_by_path("/root/shared/x/b/c").await.unwrap();
        
            let ret = op_env.remove_with_path("/root/shared/x/b/d", None).await.unwrap();
            assert_eq!(ret, None);
                
            let _root = op_env.commit().await.unwrap();
        
            let costs = begin.elapsed().as_millis() as u64;
            Stat::write(zone, LOCAL_CACHE_MAP, costs).await;
        }
        {
            let begin = std::time::Instant::now();
            // create_path_op_env None access默认权限操作自己dec_id
            let local_cache = device1.local_cache_stub(None);
            let op_env = local_cache.create_path_op_env().await.unwrap();
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
    
            let _root = op_env.commit().await.unwrap();
    
            let costs = begin.elapsed().as_millis() as u64;
            Stat::write(zone, LOCAL_CACHE_SET, costs).await;
        }

    }

}

async fn open_access(stack: &SharedCyfsStack, dec_id: &ObjectId, req_path: impl Into<String>, perm: AccessPermissions) {
    // 权限控制acl root_state meta => rmeta
    let meta = stack.root_state_meta_stub(None, None);
    // 为当前Zone内的desc_id开放req_path的读写权限
    let item = GlobalStatePathAccessItem {
        path: req_path.into(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            zone_category: Some(DeviceZoneCategory::CurrentZone),
            dec: Some(dec_id.clone()),
            access: perm as u8,
        }),
    };

    meta.add_access(item).await.unwrap();

}