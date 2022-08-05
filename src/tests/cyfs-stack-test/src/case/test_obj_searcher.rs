use cyfs_base::*;
use cyfs_lib::*;
use cyfs_stack_loader::*;
use zone_simulator::*;

use std::str::FromStr;

fn gen_random_device() -> (DeviceId, Device) {
    let area = Area::new(0, 0, 0, 0);

    let private_key = PrivateKey::generate_rsa(1024).unwrap();

    let pubic_key = private_key.public();

    let endpoints = vec![Endpoint::default()];
    let sn_list = vec![];

    let sn_unique_id = UniqueId::default();
    let device = Device::new(
        Some(ObjectId::default()),
        sn_unique_id,
        endpoints,
        sn_list,
        Vec::new(),
        pubic_key,
        area,
        DeviceCategory::Server,
    )
    .build();

    let device_id = device.desc().device_id();

    (device_id, device)
}

pub async fn test() {
    let ood1 = TestLoader::get_stack(DeviceIndex::User1OOD);
    let device1 = TestLoader::get_stack(DeviceIndex::User1Device1);
    let device2 = TestLoader::get_stack(DeviceIndex::User1Device2);

    let device_id = DeviceId::from_str("5hLXAcQGSBudJr3zLJv4bGXrv8jfA3zzqQW3ZoYEqYot").unwrap();

    let ret = test_device_search(&device1, &device_id).await;
    assert!(ret.is_err());

    let ret = test_device_search(&ood1, &device_id).await;
    assert!(ret.is_err());

    let (test_device_id, test_device) = gen_random_device();
    info!("will test device search with random device_id: {}", test_device_id);
    
    let ood1_ss = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    put_test_device(&ood1_ss, test_device).await;

    let ret = test_device_search(&device1, &test_device_id).await;
    assert!(ret.is_ok());

    let ret = test_device_search(&ood1, &test_device_id).await;
    assert!(ret.is_ok());

    let ret = test_device_search(&device2, &test_device_id).await;
    assert!(ret.is_ok());
}

async fn test_device_search(stack: &CyfsStack, device_id: &DeviceId) -> BuckyResult<Device> {
    stack.device_manager().search_device(&device_id).await
}

async fn put_test_device(stack: &SharedCyfsStack, device: Device) {
    let object_raw = device.to_vec().unwrap();
    let object_id = device.desc().device_id().object_id().to_owned();

    let req = NONPutObjectOutputRequest::new_noc(object_id.clone(), object_raw);

    let ret = stack.non_service().put_object(req).await;
    match ret {
        Err(e) => {
            unreachable!("{}", e);
        }
        Ok(_) => {
            info!("put test device to local success! id={}", object_id);
        }
    };
}
