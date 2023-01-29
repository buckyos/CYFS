use std::path::Path;
use futures::AsyncWriteExt;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

pub async fn test() {
    get_version_info().await;
    get_device_static_info().await;
    get_network_access_info().await;
    get_noc_stat().await;
    get_ood_status().await;
    test_zone().await;
    get_system_info().await;
    build_file().await;


    info!("test all util case success!");
}

async fn get_device_static_info() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let req = UtilGetDeviceStaticInfoRequest::new();
    let resp = user1_ood.util().get_device_static_info(req).await.unwrap();
    info!("{}", resp);

    let ood_info = resp.info;
    assert_eq!(ood_info.ood_device_id, USER1_DATA.get().unwrap().ood_id);
    assert!(ood_info.is_ood_device);

    let req = UtilGetDeviceStaticInfoRequest::new();
    let resp = user1_device1
        .util()
        .get_device_static_info(req.clone())
        .await
        .unwrap();
    let device1_info = resp.info;
    info!("{:?}", device1_info);

    assert_eq!(device1_info.ood_device_id, USER1_DATA.get().unwrap().ood_id);
    assert_eq!(device1_info.zone_id, ood_info.zone_id);
    assert!(!device1_info.is_ood_device);

    // 同zone跨设备读取信息
    let mut req = UtilGetDeviceStaticInfoRequest::new();
    let ood_id = USER1_DATA.get().unwrap().ood_id.clone();
    req.common.target = Some(ood_id.object_id().clone());
    let ood_info2 = user1_device1
        .util()
        .get_device_static_info(req.clone())
        .await
        .unwrap();
    info!("{}", ood_info2);

    assert_eq!(ood_info2.info.device_id, ood_info.device_id);
    assert_eq!(
        ood_info2.info.device.to_vec().unwrap(),
        ood_info.device.to_vec().unwrap()
    );
    assert_eq!(ood_info2.info.ood_device_id, ood_info.ood_device_id);
    assert_eq!(ood_info2.info.zone_id, ood_info.zone_id);
    assert_eq!(ood_info2.info.owner_id, ood_info.owner_id);
    assert_eq!(ood_info2.info.cyfs_root, ood_info.cyfs_root);

    // 跨zone读取信息
    let ret = user2_device1
        .util()
        .get_device_static_info(req)
        .await;
    assert!(ret.is_err());
    assert!(ret.unwrap_err().code() == BuckyErrorCode::PermissionDenied);
}

async fn get_network_access_info() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let req = UtilGetNetworkAccessInfoRequest::new();
    let ood_info = user1_ood
        .util()
        .get_network_access_info(req.clone())
        .await
        .unwrap();
    info!("{}", ood_info);

    let device1_info = user1_device1
        .util()
        .get_network_access_info(req)
        .await
        .unwrap();
    info!("{}", device1_info);

    let ood_id = USER1_DATA.get().unwrap().ood_id.clone();
    let mut req = UtilGetNetworkAccessInfoRequest::new();
    req.common.target = Some(ood_id.object_id().to_owned());
    let ood_info2 = user1_device1
        .util()
        .get_network_access_info(req.clone())
        .await
        .unwrap();
    info!("{}", ood_info2);

    assert_eq!(ood_info.info.v4, ood_info2.info.v4);
    assert_eq!(ood_info.info.v6, ood_info2.info.v6);
    assert_eq!(ood_info.info.sn, ood_info2.info.sn);

    // 跨zone读取信息
    let ret = user2_device1
        .util()
        .get_network_access_info(req)
        .await;
    assert!(ret.is_err());
    assert!(ret.unwrap_err().code() == BuckyErrorCode::PermissionDenied);
}

async fn get_noc_stat() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let req = UtilGetNOCInfoRequest::new();
    let ood_info = user1_ood.util().get_noc_info(req.clone()).await.unwrap();
    info!("{}", ood_info);

    let device1_info = user1_device1.util().get_noc_info(req).await.unwrap();
    info!("{}", device1_info);

    let ood_id = USER1_DATA.get().unwrap().ood_id.clone();
    let mut req = UtilGetNOCInfoRequest::new();
    req.common.target = Some(ood_id.object_id().to_owned());
    let ood_info2 = user1_device1
        .util()
        .get_noc_info(req.clone())
        .await
        .unwrap();
    info!("{}", ood_info2);

    assert_eq!(ood_info.stat.count, ood_info2.stat.count);
    assert_eq!(ood_info.stat.storage_size, ood_info2.stat.storage_size);

    // 跨zone读取信息
    let ret = user2_device1
        .util()
        .get_noc_info(req)
        .await;
    assert!(ret.is_err());
    assert!(ret.unwrap_err().code() == BuckyErrorCode::PermissionDenied);
}

async fn get_ood_status() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let _user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let req = UtilGetOODStatusRequest::new();
    let ood_info = user1_device1.util().get_ood_status(req.clone()).await.unwrap();
    info!("{}", ood_info);

    let ret = user1_ood.util().get_ood_status(req).await;
    assert!(ret.is_err());
}

async fn test_zone() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let _user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let req = UtilGetDeviceRequest::new();
    let resp = user1_ood.util().get_device(req).await.unwrap();
    assert_eq!(resp.device_id, USER1_DATA.get().unwrap().ood_id);
    assert_eq!(resp.device_id, resp.device.desc().device_id());
    assert_eq!(
        resp.device.desc().owner().unwrap().to_string(),
        USER1_DATA.get().unwrap().people_id.to_string()
    );
    let ood_device_id1 = resp.device_id;

    let req = UtilGetDeviceRequest::new();
    let resp = user1_device1.util().get_device(req).await.unwrap();
    assert_eq!(resp.device_id, USER1_DATA.get().unwrap().device1_id);
    assert_eq!(resp.device_id, resp.device.desc().device_id());
    assert_eq!(
        resp.device.desc().owner().unwrap().to_string(),
        USER1_DATA.get().unwrap().people_id.to_string()
    );

    let req = UtilGetZoneRequest::new(None, None);
    let resp = user1_ood.util().get_zone(req.clone()).await.unwrap();
    let resp1 = user1_device1.util().get_zone(req).await.unwrap();
    assert_eq!(resp.zone_id, resp1.zone_id);
    assert_eq!(resp.device_id, resp1.device_id);
    assert_eq!(resp.device_id, ood_device_id1);
    assert_eq!(*resp.zone.ood(), ood_device_id1);
    assert_eq!(*resp1.zone.ood(), ood_device_id1);
}

async fn get_system_info() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let req = UtilGetSystemInfoRequest::new();
    let system_info = user1_ood.util().get_system_info(req).await.unwrap();
    info!("{}", system_info);

    // 同zone跨设备读取信息
    let ood_id = USER1_DATA.get().unwrap().ood_id.clone();
    let mut req = UtilGetSystemInfoRequest::new();
    req.common.target = Some(ood_id.object_id().to_owned());

    let system_info2 = user1_device1
        .util()
        .get_system_info(req)
        .await
        .unwrap();
    info!("{}", system_info2);

    // 跨zone读取信息
    let mut req = UtilGetSystemInfoRequest::new();
    req.common.target = Some(ood_id.object_id().to_owned());

    let ret = user2_device1
        .util()
        .get_system_info(req)
        .await;
    assert!(ret.is_err());
    assert!(ret.unwrap_err().code() == BuckyErrorCode::PermissionDenied);
}

async fn get_version_info() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    //let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    //let user2_device1 = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    let req = UtilGetVersionInfoRequest::new();
    let version_info = user1_ood.util().get_version_info(req).await.unwrap();
    info!("user1_ood version: {}", version_info);

    let user1_device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let mut req = UtilGetVersionInfoRequest::new();
    req.common.target = Some(user1_device1.local_device_id().object_id().to_owned());

    let version_info = user1_ood.util().get_version_info(req).await.unwrap();
    info!("{}", version_info);
}

async fn gen_random_file(local_path: &Path) {
    if local_path.exists() {
        assert!(local_path.is_file());
        std::fs::remove_file(&local_path).unwrap();
    }

    let mut opt = async_std::fs::OpenOptions::new();
    opt.write(true).create(true).truncate(true);

    let mut f = opt.open(&local_path).await.unwrap();
    let mut buf: Vec<u8> = Vec::with_capacity(1024 * 1024);
    for _ in 0..1024 {
        let buf_k: Vec<u8> = (0..1024).map(|_| rand::random::<u8>()).collect();
        buf.extend_from_slice(&buf_k);
    }

    for _i in 0..20 {
        f.write_all(&buf).await.unwrap();
    }
    f.flush().await.unwrap();
}

async fn build_file() {
    let tmp_file = std::env::temp_dir().join("test_build_file2");
    gen_random_file(tmp_file.as_path()).await;

    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);

    for _ in 0..5 {
        let stack = user1_ood.clone();
        let local_path = tmp_file.clone();
        async_std::task::spawn(async move {
            let resp = stack.util().build_file_object(UtilBuildFileOutputRequest {
                common: UtilOutputRequestCommon {
                    req_path: None,
                    dec_id: Some(ObjectId::default()),
                    target: None,
                    flags: 0
                },
                local_path,
                owner: Default::default(),
                chunk_size: 4 * 1024 * 1024,
                access: None,
            }).await.unwrap();
            info!("build file {}", resp.object_id.to_string());
        });
    }
}
