use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;


pub async fn test() {
    test_orphan_zone().await;
}

async fn test_orphan_zone() {
    // 创建一个临时的device

    let area = Area::new(0, 0, 0, 0);
    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let pubic_key = private_key.public();

    let device = Device::new(
        Some(ObjectId::default()),
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        pubic_key,
        area,
        DeviceCategory::Server,
    )
    .build();

    let device_id = device.desc().device_id();
    let req = UtilGetZoneRequest::new(Some(device_id.object_id().to_owned()), None);
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let resp = stack.util().get_zone(req.clone()).await.unwrap();
    let resp2 = stack.util().get_zone(req).await.unwrap();
   
    let zone_id = resp.zone.zone_id();
    let zone_id2 = resp2.zone.zone_id();

    assert_eq!(zone_id, zone_id2);
}