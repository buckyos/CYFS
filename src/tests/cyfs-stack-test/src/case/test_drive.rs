use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

use async_std::prelude::*;
use std::str::FromStr;

pub async fn test() {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    get_file(&stack).await;

    info!("test all drive case success!");
}

async fn get_file(stack: &SharedCyfsStack) {
    let dec_id = DecAppId::from_str("9tGpLNnSx4GVQTqg5uzUucPbK1TNJdZk3nNA77PPJaPW").unwrap();
    let dir_id = DirId::from_str("7jMmeXZiN6YtJZK9QxGN3DTC9b7CR5FEsdnReMFB592N").unwrap();
    //let target = DeviceId::from_str("5bnZHzZZPqJzvApGaGSQSCfYcKiGxxQVJ54ADiaizDky").unwrap();
    let target = DeviceId::from_str("5aSixgLsQcYeHpjLZGTJxepNFDETgUN3JPpLYohcxLqr").unwrap();

    //let ood_stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    //let target = stack.local_device_id();

    let mut req = NDNGetDataRequest::new(
        NDNAPILevel::Router,
        dir_id.object_id().to_owned(),
        Some("ood-daemon_849_rCURRENT.log".to_owned()),
    );
    req.common.req_path = Some("drive".to_owned());
    req.common.target = Some(target.object_id().to_owned());
    req.common.dec_id = Some(dec_id.object_id().to_owned());

    let mut resp = stack.ndn_service().get_data(req).await.unwrap();
    info!("get_file success! len={}", resp.length);

    // 保存到本地文件
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("drive");
    std::fs::create_dir_all(&data_dir).unwrap();

    let path = data_dir.join("ood-daemon_849_rCURRENT.log");
    let mut file = async_std::fs::File::create(path).await.unwrap();

    let mut buf = vec![0; 1024 * 1024];
    loop {
        let len = resp.data.read(&mut buf).await.unwrap();
        if len == 0 {
            break;
        }

        let wlen = file.write(&buf[0..len]).await.unwrap();
        assert_eq!(len, wlen);
    }
}
