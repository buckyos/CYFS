use cyfs_base::*;
use cyfs_lib::*;
use zone_simulator::*;


async fn change_active_ood(stack: &SharedCyfsStack) {
    let mut owner = USER1_DATA.get().unwrap().user.people.clone();
    info!("current's owner={}", owner.format_json().to_string());

    {
        let ood_list = owner.body_mut().as_mut().unwrap().content_mut().ood_list_mut();
        ood_list.swap(0, 1);
    }
    owner
        .body_mut()
        .as_mut()
        .unwrap()
        .increase_update_time(bucky_time_now());

    // first post without sign, and will fail
    let buf = owner.to_vec().unwrap();
    let req = NONPostObjectOutputRequest::new_router(
        Some(stack.local_device_id().object_id().to_owned()),
        owner.desc().calculate_id(),
        buf,
    );

    let resp = stack.non_service().post_object(req).await;
    assert!(resp.is_err());

    // with sign
    let signer = RsaCPUObjectSigner::new(USER1_DATA.get().unwrap().user.sk.public(), USER1_DATA.get().unwrap().user.sk.clone());
    cyfs_base::sign_and_set_named_object_body(
        &signer,
        &mut owner,
        &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF),
    )
    .await
    .unwrap();

    let buf = owner.to_vec().unwrap();
    let req = NONPostObjectOutputRequest::new_router(
        Some(stack.local_device_id().object_id().to_owned()),
        owner.desc().calculate_id(),
        buf,
    );
    let resp = stack.non_service().post_object(req).await.unwrap();
    assert!(resp.object.is_none());

    info!("change active ood success!");
}

pub async fn test() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    change_active_ood(&user1_ood).await;

    async_std::task::sleep(std::time::Duration::from_secs(20)).await;


    info!("test all role case success!")
}
