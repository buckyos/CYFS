use cyfs_base::*;
use ood_control::*;

use qrcode::{QrCode, render::unicode::Dense1x2, EcLevel};


const BIND_CODE: &str = r#"
{
    "flag":"cyfs",
    "type":"bindOOD",
    "data":{
      "type": "OOD",
      "ip": ${ip_list},
      "access_token": "${access_token}"
    }
}
"#;
pub struct ActivateControl {}

impl ActivateControl {
    pub async fn run(tcp_port: Option<u16>, tcp_host: Option<ControlTCPHost>, addr_type: ControlInterfaceAddrType) -> BuckyResult<()> {
        info!(
            "ood activate control: host={:?}, port={:?}, type={:?}",
            tcp_port, tcp_host, addr_type,
        );

        if Self::check_bind() {
            println!("OOD bind already!");
            return Ok(());
        }

        println!(
            "will run ood activate control service... \nport: {:?}, \nhost: {:?}, \ntype: {:?}",
            tcp_port, tcp_host, addr_type,
        );

        let tcp_port = tcp_port.unwrap_or(cyfs_base::OOD_INSTALLER_CONTROL_PORT);
        let param = ControlInterfaceParam {
            mode: OODControlMode::Daemon,
            tcp_port: Some(tcp_port),
            require_access_token: true,
            tcp_host,
            addr_type,
        };

        let control_interface = ControlInterface::new(param, &OOD_CONTROLLER);
        if let Err(e) = control_interface.start().await {
            return Err(e);
        }

        let access_info = control_interface.get_access_info();
        println!("ood activate control service launched");
        println!(
            "access_token: {}",
            access_info.access_token.as_ref().unwrap()
        );
        println!("bind connect addr: {:?}", access_info.addrs);

        Self::display_qrcode(&access_info);

        Self::wait_activate().await;

        Ok(())
    }

    fn display_qrcode(access_info: &ControlInterfaceAccessInfo) {
        let ip_list: Vec<String> = access_info
            .addrs
            .iter()
            .map(|addr| format!(r#""{}""#, addr.to_string()))
            .collect();
        let ip_list = format!("[{}]", ip_list.join(","));
        let bind_code = BIND_CODE.replace("${ip_list}", &ip_list).replace(
            "${access_token}",
            access_info.access_token.as_ref().unwrap(),
        );

        info!("bind code: {}, len={}", bind_code, bind_code.len());

        println!("{}", bind_code);

        let code = QrCode::with_error_correction_level(&bind_code, EcLevel::L).unwrap();
        let image = code.render::<Dense1x2>().module_dimensions(1, 1).build();

        println!("{}", image);
    }

    fn check_bind() -> bool {
        if !OOD_CONTROLLER.is_bind() {
            return false;
        }

        let info = OOD_CONTROLLER.fill_bind_info();
        println!("device: {}", info.device_id);
        println!("zone's owner: {}", info.owner_id);
        true
    }

    async fn wait_activate() {
        loop {
            if Self::check_bind() {
                info!("ood bind success!");
                println!("OOD bind success!");
                break;
            }

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}
