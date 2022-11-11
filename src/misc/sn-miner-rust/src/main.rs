mod init_log;

use std::collections::LinkedList;
use std::io::{Read, Write};
use std::path::Path;
use std::str::FromStr;

use cyfs_base::*;
use cyfs_bdt::{sn::service::*, ReceiptWithSignature, SnServiceReceipt};
use cyfs_debug::DebugConfig;

const APP_NAME: &str = "sn-miner";

struct SnServiceContractServerImpl {}

impl SnServiceContractServerImpl {
    fn new() -> SnServiceContractServerImpl {
        SnServiceContractServerImpl {}
    }
}

impl SnServiceContractServer for SnServiceContractServerImpl {
    fn check_receipt(
        &self,
        _client_device: &Device,
        _local_receipt: &SnServiceReceipt,
        _client_receipt: &Option<ReceiptWithSignature>,
        _last_request_time: &ReceiptRequestTime,
    ) -> IsAcceptClient {
        IsAcceptClient::Accept(false)
    }

    fn verify_auth(&self, _client_device_id: &DeviceId) -> IsAcceptClient {
        IsAcceptClient::Accept(false)
    }
}

fn get_ep_list() -> Vec<Endpoint> {
    let mut eps = vec![];
    match cyfs_base::get_channel() {
        CyfsChannel::Nightly => {
            eps.push(Endpoint::from_str("W4tcp120.25.76.67:8060").unwrap());
            eps.push(Endpoint::from_str("W4udp120.25.76.67:8060").unwrap());
            eps.push(
                Endpoint::from_str("W6tcp[2408:4003:108b:ba00:98ab:b856:5f0c:4171]:8061").unwrap(),
            );
            eps.push(
                Endpoint::from_str("W6udp[2408:4003:108b:ba00:98ab:b856:5f0c:4171]:8061").unwrap(),
            );
        }
        CyfsChannel::Beta => {
            eps.push(Endpoint::from_str("W4tcp120.24.6.201:8060").unwrap());
            eps.push(Endpoint::from_str("W4udp120.24.6.201:8060").unwrap());
            eps.push(
                Endpoint::from_str("W6tcp[2408:4003:108b:ba00:98ab:b856:5f0c:4170]:8061").unwrap(),
            );
            eps.push(
                Endpoint::from_str("W6udp[2408:4003:108b:ba00:98ab:b856:5f0c:4170]:8061").unwrap(),
            );
        }
        CyfsChannel::Stable => {
            unreachable!()
        }
    }
    eps
}

#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec(APP_NAME);

    cyfs_debug::CyfsLoggerBuilder::new_app(APP_NAME)
        .level("info")
        .console("warn")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new(APP_NAME, APP_NAME)
        .exit_on_panic(true)
        .build()
        .start();

    let data_folder = ::cyfs_util::get_app_data_dir(APP_NAME);
    let (local_device, private_key) = if let Ok(loaded_device) =
        load_device_info(data_folder.as_path())
    {
        /*
            let old_ep_list: &mut Vec<Endpoint> = loaded_device.0.mut_connect_info().mut_endpoints();
            let mut ep_changed = old_ep_list.len() != ep_list.len();
            if !ep_changed {
                for ep in ep_list.as_slice() {
                    let mut exist = false;
                    for old_ep in &mut *old_ep_list {
                        if old_ep == ep {
                            exist = true;
                            break;
                        }
                    }
                    if !exist {
                        ep_changed = true;
                        break;
                    }
                }
            }

            ep_changed = false; // 一直用desc配置文件里的endpoint
            if ep_changed {
                old_ep_list.clear();
                old_ep_list.append(&mut ep_list);

                let mut encode_buffer = vec![];
                encode_buffer.resize(loaded_device.0.raw_measure(&None).unwrap(), 0);
                let remain_len = loaded_device.0.raw_encode(encode_buffer.as_mut_slice(), &None).unwrap().len();
                encode_buffer.truncate(encode_buffer.len() - remain_len);

                let mut file_path = data_folder.to_path_buf();
                file_path.push(APP_NAME.to_owned() + ".desc");
                let _ = save_to_file(file_path.as_path(), encode_buffer.as_slice());
            }
        */
        log::info!(
            "sn-miner load device success, path is: {}.",
            data_folder.to_str().unwrap()
        );
        loaded_device
    } else {
        log::info!(
            "sn-miner no device load from {}.",
            data_folder.to_str().unwrap()
        );
        let ep_list = get_ep_list();
        // <TODO>暂时没有安装流程，就直接生成一个，按道理应该有个desc导入导出逻辑
        match create_device_info(data_folder.as_path(), ep_list).await {
            Ok(new_device) => {
                log::info!(
                    "sn-miner create desc success. and saved at {}.",
                    data_folder.to_str().unwrap()
                );
                new_device
            }
            Err(e) => {
                log::error!("sn-miner startup failed for the file (sn_miner.desc/sec) load/create failed. err: {:?}, path is: {}.", e, data_folder.to_str().unwrap());
                return;
            }
        }
    };

    let service = SnService::new(
        local_device,
        private_key,
        Box::new(SnServiceContractServerImpl::new()),
    );

    let _ = service.start().await;

    println!("exit.");
}

fn load_device_info(folder_path: &Path) -> Result<(Device, PrivateKey), BuckyError> {
    let mut file_path = folder_path.to_path_buf();
    file_path.push(APP_NAME.to_owned() + ".desc");

    let mut file = std::fs::File::open(file_path)?;
    let mut buf = Vec::<u8>::new();
    file.read_to_end(&mut buf)?;
    let (device, _) = Device::raw_decode(buf.as_slice())?;

    let mut private_path = folder_path.to_path_buf();
    private_path.push(APP_NAME.to_owned() + ".sec");

    let mut file = std::fs::File::open(private_path)?;
    let mut buf = Vec::<u8>::new();
    file.read_to_end(&mut buf)?;
    let (private_key, _) = PrivateKey::raw_decode(buf.as_slice())?;

    Ok((device, private_key))
}

async fn create_device_info(
    folder_path: &Path,
    ep_list: Vec<Endpoint>,
) -> Result<(Device, PrivateKey), BuckyError> {
    let unique_name = "bucky-sn-server";
    let mut unique_id = [0u8; 16];
    unique_id[..unique_name.len()].copy_from_slice(unique_name.as_bytes());

    let private_key = PrivateKey::generate_rsa(1024).unwrap();

    let device = Device::new(
        None,
        UniqueId::clone_from_slice(&unique_id),
        ep_list,
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::Server,
    )
    .build();

    let mut encode_buffer = vec![];
    encode_buffer.resize(device.raw_measure(&None).unwrap(), 0);
    let remain_len = device
        .raw_encode(encode_buffer.as_mut_slice(), &None)?
        .len();
    encode_buffer.truncate(encode_buffer.len() - remain_len);

    let mut file_path = folder_path.to_path_buf();
    file_path.push(APP_NAME.to_owned() + ".desc");
    save_to_file(file_path.as_path(), encode_buffer.as_slice())?;

    encode_buffer.resize(private_key.raw_measure(&None)?, 0);
    let remain_len = private_key
        .raw_encode(encode_buffer.as_mut_slice(), &None)?
        .len();
    encode_buffer.truncate(encode_buffer.len() - remain_len);

    let mut file_path = folder_path.to_path_buf();
    file_path.push(APP_NAME.to_owned() + ".sec");
    save_to_file(file_path.as_path(), encode_buffer.as_slice())?;

    Ok((device, private_key))
}

fn save_to_file(file_path: &Path, buf: &[u8]) -> Result<(), BuckyError> {
    let mut file = std::fs::File::create(file_path)?;
    file.write(buf).map(|_| ()).map_err(|e| BuckyError::from(e))
}
