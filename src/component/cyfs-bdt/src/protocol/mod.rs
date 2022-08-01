#[macro_export]

macro_rules! downcast_sn_handle {
    ($dynamic_package: expr, $handler: expr) => {
        match $dynamic_package.cmd_code() {
            protocol::PackageCmdCode::SnCall => $handler($dynamic_package.as_any().downcast_ref::<protocol::SnCall>().unwrap()),
            protocol::PackageCmdCode::SnCallResp => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::SnCallResp>().unwrap()),
            protocol::PackageCmdCode::SnCalled => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::SnCalled>().unwrap()),
            protocol::PackageCmdCode::SnCalledResp => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::SnCalledResp>().unwrap()),
            protocol::PackageCmdCode::SnPing => $handler($dynamic_package.as_any().downcast_ref::<protocol::SnPing>().unwrap()),
            protocol::PackageCmdCode::SnPingResp => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::SnPingResp>().unwrap()),
            _ => panic!()
        }
    }
}

macro_rules! downcast_tunnel_handle {
    ($dynamic_package: expr, $handler: expr) => {
        match $dynamic_package.cmd_code() {
            protocol::PackageCmdCode::SynTunnel => $handler($dynamic_package.as_any().downcast_ref::<protocol::SynTunnel>().unwrap()),
            protocol::PackageCmdCode::AckTunnel => $handler($dynamic_package.as_any().downcast_ref::<protocol::AckTunnel>().unwrap()),
            protocol::PackageCmdCode::AckAckTunnel => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::AckAckTunnel>().unwrap()),
            protocol::PackageCmdCode::PingTunnel => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::PingTunnel>().unwrap()),
            protocol::PackageCmdCode::PingTunnelResp => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::PingTunnelResp>().unwrap()),
            protocol::PackageCmdCode::Datagram => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::Datagram>().unwrap()),
            protocol::PackageCmdCode::SessionData => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::SessionData>().unwrap()),
            protocol::PackageCmdCode::TcpSynConnection => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::TcpSynConnection>().unwrap()),
            _ => panic!()
        }
    }
}

macro_rules! downcast_tcp_stream_handle {
    ($dynamic_package: expr, $handler: expr) => {
        match $dynamic_package.cmd_code() {
            protocol::PackageCmdCode::TcpSynConnection => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::TcpSynConnection>().unwrap()),
            protocol::PackageCmdCode::TcpAckConnection => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::TcpAckConnection>().unwrap()),
            protocol::PackageCmdCode::TcpAckAckConnection => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::TcpAckAckConnection>().unwrap()),
            _ => panic!()
        }
    }
}

macro_rules! downcast_session_handle {
    ($dynamic_package: expr, $handler: expr) => {
        match $dynamic_package.cmd_code() {
            protocol::PackageCmdCode::Datagram => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::Datagram>().unwrap()),
            protocol::PackageCmdCode::SessionData => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::SessionData>().unwrap()),
            protocol::PackageCmdCode::TcpSynConnection => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::TcpSynConnection>().unwrap()),
            _ => panic!()
        }
    }
}

macro_rules! downcast_proxy_handle {
    ($dynamic_package: expr, $handler: expr) => {
        match $dynamic_package.cmd_code() {
            protocol::PackageCmdCode::SynProxy => $handler($dynamic_package.as_any().downcast_ref::<protocol::SynProxy>().unwrap()),
            protocol::PackageCmdCode::AckProxy => $handler($dynamic_package.as_any().downcast_ref::<protocol::v0::AckProxy>().unwrap()),
            _ => panic!()
        }
    }
}


macro_rules! downcast_handle {
    ($dynamic_package: expr, $handler: expr) => {
        if $dynamic_package.cmd_code().is_exchange() {
            $handler($dynamic_package.as_any().downcast_ref::<protocol::Exchange>().unwrap())
        } else if $dynamic_package.cmd_code().is_tunnel() {
            downcast_tunnel_handle!($dynamic_package, $handler)
        } else if $dynamic_package.cmd_code().is_sn() {
            downcast_sn_handle!($dynamic_package, $handler)
        } else if $dynamic_package.cmd_code().is_proxy() {
            downcast_proxy_handle!($dynamic_package, $handler)
        } else {
            downcast_tcp_stream_handle!($dynamic_package, $handler)
        }
    };
    ($dynamic_package: expr) => {
        downcast_handle!($dynamic_package, |p| p)
    };
}

mod common;
mod package;
mod package_box;
pub mod v0;

pub use common::*;
pub use package::*;
pub use package_box::*;
