use clap::{App, Arg};
use cyfs_base::{
    Area, BuckyError, BuckyErrorCode, BuckyResult, Device, DeviceCategory, Endpoint, FileDecoder,
    FileEncoder, NamedObject, ObjectDesc, ObjectId, PrivateKey, Protocol, UniqueId,
};
use log::*;
use std::time::Duration;
use rand::Rng;

// sn-online-test.exe <sn-device-path> <timeout secs>
// 成功返回0，不成功返回其他值
#[async_std::main]
async fn main() -> BuckyResult<()> {
    simple_logger::init_with_level(Level::Info).unwrap();
    let matches = App::new("sn-online-test")
        .arg(
            Arg::with_name("sn-device-path")
                .index(1)
                .takes_value(true)
                .required(true),
        )
        .arg(Arg::with_name("timeout").index(2).default_value("10"))
        .get_matches();

    let (sn_device, _) = Device::decode_from_file(matches.value_of("sn-device-path").unwrap().as_ref(), &mut vec![])?;
    let sn_id = sn_device.desc().calculate_id();
    let timeout_secs = matches.value_of("timeout").unwrap().parse()?;

    let data_path = cyfs_util::get_service_data_dir("sn-online-test").join("device");
    std::fs::create_dir_all(data_path.parent().unwrap()).unwrap();
    let desc_path = data_path.with_extension("desc");
    let sec_path = data_path.with_extension("sec");
    let (mut device, secret) = if desc_path.exists() && sec_path.exists() {
        let (device_sec, _) = PrivateKey::decode_from_file(&sec_path, &mut vec![])?;
        let (device, _) = Device::decode_from_file(&desc_path, &mut vec![])?;
        (device, device_sec)
    } else {
        let device_sec = PrivateKey::generate_secp256k1()?;

        let device = Device::new(
            None,
            UniqueId::create("sn-online-test".as_bytes()),
            vec![],
            vec![],
            vec![],
            device_sec.public(),
            Area::default(),
            DeviceCategory::Server,
        )
        .build();
        device_sec.encode_to_file(&sec_path, false)?;
        device.encode_to_file(&desc_path, false)?;
        (device, device_sec)
    };

    // 测试起一个单独的bdt栈，等待它上线
    let device_id = device.desc().calculate_id();
    info!("current device_id: {}", device_id);

    // desc.endpoints.clear();
    let endpoints = device
        .body_mut()
        .as_mut()
        .unwrap()
        .content_mut()
        .mut_endpoints();
    if endpoints.len() == 0 {
        // 取随机端口号
        let port = rand::thread_rng().gen_range(30000, 50000) as u16;
        for ip in cyfs_util::get_all_ips().unwrap() {
            if ip.is_ipv4() {
                endpoints.push(Endpoint::from((Protocol::Tcp, ip, port)));
                endpoints.push(Endpoint::from((Protocol::Udp, ip, port)));
            }
        }
    }

    let mut params = cyfs_bdt::StackOpenParams::new("sn-online-test");
    params.known_sn = Some(vec![sn_device.clone()]);

    let bdt_stack = cyfs_bdt::Stack::open(device, secret, params).await?;
    bdt_stack.reset_sn_list(vec![sn_device.clone()]);

    // 创建bdt-stack协议栈后等待bdt-stack在SN上线
    info!(
        "now device {} will wait for sn {} online.",
        bdt_stack.local_device_id(),
        &sn_id
    );
    async_std::future::timeout(Duration::from_secs(timeout_secs), async {
        let ret = bdt_stack.sn_client().ping().wait_online().await;
        if let Err(e) = ret {
            let msg = format!(
                "bdt stack wait sn online failed! device {}, sn {}, err {}",
                bdt_stack.local_device_id(),
                &sn_id,
                e
            );
            return Err(BuckyError::new(BuckyErrorCode::ConnectFailed, msg));
        } else {
            info!(
                "bdt stack sn online success! device {}, sn {}",
                bdt_stack.local_device_id(),
                &sn_id
            );
            Ok(())
        }
    })
    .await??;

    Ok(())
}
