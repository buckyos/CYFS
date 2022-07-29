use async_std::task;
use clap::{App, Arg};
use cyfs_base::*;
use cyfs_bdt::pn::{self, service::ProxyServiceEvents};
use std::{
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    str::FromStr,
    time::Duration,
};
mod auth;

const APP_NAME: &str = "pn-miner";

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

#[async_std::main]
async fn main() {
    let command = App::new(APP_NAME)
        .about("pn miner demo")
        .version(cyfs_base::get_version())
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("port")
                .help("auth server port")
                .default_value("80"),
        );
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

    let matches = command.get_matches();
    let auth_port = u16::from_str(matches.value_of("port").unwrap())
        .map_err(|err| log::error!("invalid auth port {}", err))
        .unwrap();

    task::block_on(async {
        let data_folder = ::cyfs_util::get_app_data_dir(APP_NAME);

        if let Ok((local_device, private_key)) = load_device_info(data_folder.as_path()) {
            log::info!(
                "pn-miner load device success, path is: {}.",
                data_folder.to_str().unwrap()
            );

            let mut local_device = local_device;
            // bind 0 地址
            let cmd_ep = &mut local_device.mut_connect_info().mut_endpoints()[0];
            let outer_addr = *cmd_ep.addr();
            cmd_ep.mut_addr().set_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED));

            //FIXME: 写死了 proxy port
            let proxy_local =
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), outer_addr.port() + 1);
            let proxy_outer = SocketAddr::new(outer_addr.ip(), proxy_local.port());

            let auth_store = if auth_port > 0 {
                let mut auth_db_path = data_folder.to_path_buf();
                auth_db_path.push("auth.db");

                Some(
                    auth::storage::Storage::new(
                        auth_db_path.as_path(),
                        auth::storage::Config {
                            bandwidth: vec![(10, 10000), (20, 10000)],
                        },
                    )
                    .unwrap(),
                )
            } else {
                None
            };

            if let Ok(service) = pn::service::Service::start(
                local_device.clone(),
                private_key,
                vec![(proxy_local, Some(proxy_outer))],
                None,
                auth_store
                    .clone()
                    .map(|s| Box::new(s) as Box<dyn ProxyServiceEvents>),
            )
            .await
            {
                log::info!("pn-miner auth server listen on {}", auth_port);
                if auth::interface::listen(auth_port, local_device.desc().device_id(), auth_store)
                    .await
                    .is_ok()
                {
                    loop {
                        let _ = task::sleep(Duration::from_secs(100000)).await;
                        log::trace!("{} active", service);
                    }
                } else {
                    log::error!("pn-miner auth server start failed");
                }
            } else {
                log::error!("pn-miner start failed");
            }
        } else {
            log::error!(
                "pn-miner load device failed, path is: {}.",
                data_folder.to_str().unwrap()
            );
        };
    });

    println!("exit.");
}
