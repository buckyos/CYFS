use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;

use cyfs_base::*;
use cyfs_bdt::{sn::service::*, ReceiptWithSignature, SnServiceReceipt};

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

#[async_std::main]
async fn main() {
    let data_folder = cyfs_util::get_app_data_dir(APP_NAME);
    let default_desc_path = data_folder.join(APP_NAME);
    let matches = clap::App::new(APP_NAME).version(cyfs_base::get_version())
        .arg(clap::Arg::with_name("desc").short("d").long("desc").takes_value(true)
            .default_value(default_desc_path.to_str().unwrap())
            .help("sn desc/sec files, exclude extension")).get_matches();

    match load_device_info(Path::new(matches.value_of("desc").unwrap())) {
        Ok((device, private_key)) => {
            let unique_id = String::from_utf8_lossy(device.desc().unique_id().as_slice());
            cyfs_debug::CyfsLoggerBuilder::new_app(APP_NAME)
                .level("info")
                .console("warn")
                .build()
                .unwrap()
                .start();

            cyfs_debug::PanicBuilder::new(APP_NAME, unique_id.as_ref())
                .exit_on_panic(true)
                .build()
                .start();

            log::info!("sn-miner load device from {}, id {}", matches.value_of("desc").unwrap(), device.desc().object_id());

            let service = SnService::new(
                device,
                private_key,
                Box::new(SnServiceContractServerImpl::new()),
            );

            let _ = service.start().await;
        }
        Err(e) => {
            println!("ERROR: read desc/sec file err {}, path {}", e, matches.value_of("desc").unwrap());
            std::process::exit(1);
        }
    }

    println!("exit.");
}

fn load_device_info(folder_path: &Path) -> BuckyResult<(Device, PrivateKey)> {
    let (mut device, _) = Device::decode_from_file(folder_path.with_extension("desc").as_path(), &mut vec![])?;
    let (private_key, _) = PrivateKey::decode_from_file(folder_path.with_extension("sec").as_path(), &mut vec![])?;

    for endpoint in device.mut_connect_info().mut_endpoints() {
        match endpoint.mut_addr() {
            SocketAddr::V4(mut addr) => {
                addr.set_ip(Ipv4Addr::UNSPECIFIED)
            }
            SocketAddr::V6(mut addr) => {
                addr.set_ip(Ipv6Addr::UNSPECIFIED)
            }
        }
    }

    Ok((device, private_key))
}
