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
        info!("begin test RootStateBench...");
        let begin = std::time::Instant::now();
        let ret = if env == BenchEnv::Simulator {
            for _ in 0..t {
                let _ret = test(zone).await;
            }
            true
        } else {
            true
        };

        let dur = begin.elapsed();
        info!("end test RootStateBench: {:?}", dur);

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
    let user1_stack = zone.get_shared_stack("zone1_ood");
    let user1_device1_stack = zone.get_shared_stack("zone1_device1");

    test_path_op_env_cross_dec(&user1_stack, &user1_device1_stack, zone).await;
    test_single_op_env_cross_dec(&user1_stack, &user1_device1_stack, zone).await;
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

// 测试root-state的同zone的跨dec操作 需要配合权限
async fn test_path_op_env_cross_dec(
    user1_stack: &SharedCyfsStack,
    user1_device1_stack: &SharedCyfsStack,
    zone: &SimZone) {
    // source_dec_id 为 user1_stack.open传入的,  target_dec_id为user1_device1 open的dec_id
    // 目前的root_state 不支持 . ..
    let target_dec_id = user1_device1_stack.dec_id().unwrap().to_owned();
    let root_state = user1_stack.root_state_stub(None, Some(target_dec_id));
    let root_info = root_state.get_current_root().await.unwrap();
    debug!("current root: {:?}", root_info);


    // 目标req_path层, dec-id开启对应的权限才可以操作
    open_access(&user1_device1_stack, &target_dec_id, "/root/shared", AccessPermissions::None).await;

    let access = RootStateOpEnvAccess::new("/", AccessPermissions::Full);   // 对跨dec路径操作这个perm才work
    let op_env = root_state.create_path_op_env_with_access(Some(access)).await.unwrap();


    let x1_value = ObjectId::from_base58("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x2_value = ObjectId::from_base58("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();
    
    info!("begin test CrossRootState...");
    let begin = std::time::Instant::now();

    // test create_new
    op_env.remove_with_path("/root/shared/new", None).await.unwrap();
    op_env
        .create_new_with_path("/root/shared/new/a", ObjectMapSimpleContentType::Map)
        .await
        .unwrap();
    op_env
        .create_new_with_path("/root/shared/new/c", ObjectMapSimpleContentType::Set)
        .await
        .unwrap();

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

    let ret = op_env.get_by_path("/root/shared/x/b/c").await.unwrap();
    assert_eq!(ret, Some(x1_value));

    let ret = op_env.remove_with_path("/root/shared/x/b/d", None).await.unwrap();
    assert_eq!(ret, None);

    let root = op_env.commit().await.unwrap();
    info!("new dec root is: {:?}", root);

    let dur = begin.elapsed();
    info!("end test CrossRootState: {:?}", dur);

    let costs = begin.elapsed().as_millis() as u64;
    // 记录下耗时到本地device
    Stat::write(zone, CROSS_LOCAL_STATE, costs).await;

    {
        info!("begin test OwnerRootState...");
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

        let root = op_env.commit().await.unwrap();
        debug!("new dec root is: {:?}", root);

        let dur = begin.elapsed();
        info!("end test OwnerRootState: {:?}", dur);
    }

    info!("test root_state complete!");

}

async fn test_single_op_env_cross_dec(
    user1_stack: &SharedCyfsStack,
    user1_device1_stack: &SharedCyfsStack,
    zone: &SimZone) {
    // source_dec_id 为 user1_stack.open传入的,  target_dec_id为user1_device1 open的dec_id
    // 目前的root_state 不支持 . ..
    let source_dec_id = user1_stack.dec_id().unwrap().to_owned();
    let target_dec_id = user1_device1_stack.dec_id().unwrap().to_owned();
    let root_state = user1_stack.root_state_stub(None, Some(target_dec_id));
    let root_info = root_state.get_current_root().await.unwrap();
    debug!("current root: {:?}", root_info);

    // 目标req_path层, dec-id开启对应的权限才可以操作
    open_access(&user1_device1_stack, &target_dec_id, "/root/shared", AccessPermissions::None).await;
    
    // work
    let access = RootStateOpEnvAccess::new("/root/shared", AccessPermissions::ReadAndWrite);
    let op_env = root_state.create_single_op_env_with_access(Some(access)).await.unwrap();
    open_access(&user1_stack, &source_dec_id, "/root/shared", AccessPermissions::None).await;

    // 初始化
    let ret = op_env.load_by_path("/root/shared").await.unwrap();

    let x1_value = ObjectId::from_base58("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x2_value = ObjectId::from_base58("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // test create_new
    // 首先移除老的值，如果存在的话
    op_env.remove_with_key("b", None).await.unwrap();
    op_env.remove_with_key("c", None).await.unwrap();

    let ret = op_env.get_by_key("b").await.unwrap();
    assert_eq!(ret, None);
    let ret = op_env.get_by_key("c").await.unwrap();
    assert_eq!(ret, None);

    op_env
        .insert_with_key("b", &x1_value)
        .await
        .unwrap();

    let ret = op_env.get_by_key("b").await.unwrap();
    assert_eq!(ret, Some(x1_value));

    let ret = op_env.remove_with_key("/d", None).await.unwrap();
    assert_eq!(ret, None);

    let root = op_env.commit().await.unwrap();
    info!("new dec root is: {:?}", root);

    {
        // create_single_op_env None access默认权限操作自己dec_id
        // 首先尝试查询一下/a/b对应的object_map，用以后续校验id是否相同
        let root_state = user1_stack.root_state_stub(None, None);

        let single_op_env = root_state.create_single_op_env().await.unwrap();
        single_op_env.load_by_path("/").await.unwrap();

        let current_b = single_op_env.get_current_root().await;

        single_op_env
        .insert_with_key("c", &x2_value)
        .await
        .unwrap();

        let test1_value = single_op_env.get_by_key("c").await.unwrap();
        assert_eq!(test1_value, Some(x2_value));

        let prev_value = single_op_env
            .set_with_key("c", &x1_value, Some(x2_value), false)
            .await
            .unwrap();
        assert_eq!(prev_value, Some(x2_value));

        // 创建新的b，但老的仍然继续有效
        let new_root = single_op_env.commit().await.unwrap();
       
        info!("dec root changed to {}", new_root);
    }

    info!("test root_state complete!");

}