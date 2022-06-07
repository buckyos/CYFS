use cyfs_base::*;
use cyfs_lib::*;
use zone_simulator::*;

async fn change_access_mode(_dec_id: &ObjectId, access_mode: GlobalStateAccessMode) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);

    let user = TestLoader::get_user(DeviceIndex::User1OOD);

    let data = AdminGlobalStateAccessModeData {
        category: GlobalStateCategory::RootState,
        access_mode,
    };

    let cmd = AdminCommand::GlobalStateAccessMode(data);

    let target = stack.local_device_id().to_owned();
    let owner = user.people.desc().calculate_id();
    let mut admin_object = AdminObject::create(owner, target.clone(), cmd.clone());

    let admin_id = admin_object.desc().calculate_id();
    let buf = admin_object.to_vec().unwrap();
    let req = NONPostObjectOutputRequest::new_router(Some(target.clone().into()), admin_id, buf);
    let resp = stack.non_service().post_object(req).await;
    if let Err(e) = resp {
        assert_eq!(e.code(), BuckyErrorCode::InvalidSignature);
    } else {
        unreachable!();
    }

    // 使用people身份签名
    let signer =
        RsaCPUObjectSigner::new(user.people.desc().public_key().to_owned(), user.sk.clone());
    cyfs_base::sign_and_push_named_object_desc(
        &signer,
        &mut admin_object,
        &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER),
    )
    .await
    .unwrap();

    let admin_id = admin_object.desc().calculate_id();
    let buf = admin_object.to_vec().unwrap();
    let req = NONPostObjectOutputRequest::new_router(Some(target.into()), admin_id, buf);
    let resp = stack.non_service().post_object(req).await.unwrap();
    assert!(resp.object.is_none());

    info!("change access mode success! {:?}", access_mode);

    let new_access_mode = get_access_mode().await;
    assert_eq!(new_access_mode, access_mode);
}

async fn get_access_mode() -> GlobalStateAccessMode {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let used1_device = TestLoader::get_shared_stack(DeviceIndex::User1Device1);

    let mut req = UtilGetDeviceStaticInfoOutputRequest::new();
    req.common.target = Some(user1_ood.local_device_id().into());

    let info = used1_device
        .util()
        .get_device_static_info(req)
        .await
        .unwrap()
        .info;
    info!("device static info: {:?}", info);

    info.root_state_access_mode
}

pub async fn test() {
    // 使用协议栈本身的dec_id
    let dec_id = TestLoader::get_dec_id();

    change_access_mode(&dec_id, GlobalStateAccessMode::Read).await;
    change_access_mode(&dec_id, GlobalStateAccessMode::Write).await;
}
