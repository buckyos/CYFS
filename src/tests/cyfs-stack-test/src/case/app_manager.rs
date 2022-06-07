use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

use std::collections::HashMap;

async fn register_app(stack: &SharedCyfsStack, dec_id: &ObjectId) {
    let info = DecIpInfo {
        name: "test_dec".to_owned(),
        ip: "192.168.100.110".to_owned(),
    };

    let mut dec_map = HashMap::new();
    dec_map.insert(dec_id.to_string(), info);

    let owner = USER1_DATA.get().unwrap().people_id.clone();

    let action = AppManagerAction::create_register_dec(
        owner.object_id().clone(),
        "192.168.100.110".to_owned(),
        dec_map,
    );

    let action_id = action.desc().calculate_id();
    let buf = action.to_vec().unwrap();
    let req = NONPostObjectOutputRequest::new_router(None, action_id, buf);
    let resp = stack.non_service().post_object(req).await.unwrap();
    assert!(resp.object.is_none());

    info!("register dec success!");
}

async fn unregister_app(stack: &SharedCyfsStack, dec_id: &ObjectId) {
    let mut dec_map = HashMap::new();
    dec_map.insert(dec_id.to_string(), "test_dec".to_owned());

    let owner = USER1_DATA.get().unwrap().people_id.clone();

    let action = AppManagerAction::create_unregister_dec(owner.object_id().clone(), dec_map);

    let action_id = action.desc().calculate_id();
    let buf = action.to_vec().unwrap();
    let req = NONPostObjectOutputRequest::new_router(None, action_id, buf);
    let resp = stack.non_service().post_object(req).await.unwrap();
    assert!(resp.object.is_none());

    info!("register dec success!");
}

pub async fn test() {
    // 使用协议栈本身的dec_id
    let dec_id = TestLoader::get_dec_id();

    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    register_app(&user1_ood, &dec_id).await;

    async_std::task::sleep(std::time::Duration::from_secs(20)).await;

    unregister_app(&user1_ood, &dec_id).await;

    async_std::task::sleep(std::time::Duration::from_secs(1)).await;
    register_app(&user1_ood, &dec_id).await;

    info!("test all app manager case success!")
}
