use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use ood_control::{
    ControlInterface, ControlInterfaceParam, OODControlMode,
    OOD_CONTROLLER,
};

use clap::ArgMatches;
use std::net::IpAddr;
use std::str::FromStr;

use crate::service::ServiceMode;

pub async fn start_control(mode: ServiceMode, matches: &ArgMatches<'_>) -> BuckyResult<()> {
    let control_mode = match mode {
        ServiceMode::Daemon => Some(OODControlMode::Daemon),
        ServiceMode::Runtime => Some(OODControlMode::Runtime),
        _ => None,
    };

    if let Some(mode) = control_mode {
        start_ood_control(mode, matches).await?;
    }

    Ok(())
}

async fn start_ood_control(mode: OODControlMode, matches: &ArgMatches<'_>) -> BuckyResult<()> {
    let tcp_port = match matches.value_of("port") {
        Some(v) => {
            let port = v.parse().map_err(|e| {
                let msg = format!("invalid port: {}, {}", v, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidParam, msg)
            })?;
            Some(port)
        }
        None => None,
    };

    let tcp_host = match matches.value_of("host") {
        Some(v) => {
            let addr = IpAddr::from_str(v).map_err(|e| {
                let msg = format!("invalid host: {}, {}", v, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidParam, msg)
            })?;
            Some(addr)
        }
        None => None,
    };

    let strict_tcp_host = match matches.value_of("strict-host") {
        Some(v) => {
            let addr = IpAddr::from_str(v).map_err(|e| {
                let msg = format!("invalid strict-host: {}, {}", v, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidParam, msg)
            })?;
            Some(addr)
        }
        None => None,
    };

    let host = if strict_tcp_host.is_some() {
        Some(ood_control::ControlTCPHost::Strict(
            strict_tcp_host.unwrap(),
        ))
    } else if tcp_host.is_some() {
        Some(ood_control::ControlTCPHost::Default(tcp_host.unwrap()))
    } else {
        None
    };

    let addr_type = if matches.is_present("ipv4_only") {
        ood_control::ControlInterfaceAddrType::V4
    } else if matches.is_present("ipv6_only") {
        ood_control::ControlInterfaceAddrType::V6
    } else {
        ood_control::ControlInterfaceAddrType::All
    };

    info!(
        "ood activate control: host={:?}, port={:?}, type={:?}",
        tcp_port, tcp_host, addr_type,
    );

    let param = ControlInterfaceParam {
        mode,
        tcp_port,
        require_access_token: true,
        tcp_host: host,
        addr_type,
    };

    let control_interface = ControlInterface::new(param, &OOD_CONTROLLER);
    if let Err(e) = control_interface.start().await {
        return Err(e);
    }

    Ok(())
}
