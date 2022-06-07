use super::get_if_addrs::get_if_addrs;
use cyfs_base::{BuckyError, BuckyResult, BuckyErrorCode};

use std::error::Error;
use std::mem;
use std::net::{IpAddr, SocketAddr, SocketAddrV4, SocketAddrV6};

#[cfg(unix)]
use async_std::net::UdpSocket;
#[cfg(unix)]
use async_std::os::unix::io::RawFd;
#[cfg(unix)]
extern crate libc;

#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, RawSocket};
#[cfg(windows)]
extern crate libc;

#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use winapi::{
    shared::minwindef::{BOOL, DWORD, FALSE, LPDWORD, LPVOID},
    um::{
        mswsock::SIO_UDP_CONNRESET,
        winsock2::{WSAGetLastError, WSAIoctl, SOCKET, SOCKET_ERROR},
    },
};

pub fn get_all_ips() -> BuckyResult<Vec<IpAddr>> {
    let mut ret = Vec::new();
    for iface in get_if_addrs()? {
        ret.push(iface.ip())
    }
    Ok(ret)
}

#[cfg(unix)]
pub fn set_socket_reuseaddr(fd: RawFd) -> Result<(), Box<dyn Error>> {
    let ret;
    unsafe {
        let optval: libc::c_int = 1;
        ret = libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &optval as *const _ as *const libc::c_void,
            mem::size_of_val(&optval) as libc::socklen_t,
        );
    }

    if ret != 0 {
        let msg = format!(
            "set_socket_reuseaddr error! ret={}, err={}",
            ret,
            async_std::io::Error::last_os_error()
        );
        error!("{}", msg);

        return Err(Box::<dyn Error>::from(msg));
    }

    Ok(())
}

#[cfg(unix)]
pub fn set_socket_reuseport(fd: RawFd) -> Result<(), Box<dyn Error>> {
    let ret;
    unsafe {
        let optval: libc::c_int = 1;
        ret = libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEPORT,
            &optval as *const _ as *const libc::c_void,
            mem::size_of_val(&optval) as libc::socklen_t,
        );
    }

    if ret != 0 {
        let msg = format!(
            "set_socket_reuseport error! ret={}, err={}",
            ret,
            async_std::io::Error::last_os_error()
        );
        error!("{}", msg);

        return Err(Box::<dyn Error>::from(msg));
    }

    Ok(())
}

#[cfg(unix)]
pub fn set_socket_keepalive(fd: RawFd) -> Result<(), Box<dyn Error>> {
    let ret;
    unsafe {
        let optval: libc::c_int = 1;
        ret = libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_KEEPALIVE,
            &optval as *const _ as *const libc::c_void,
            mem::size_of_val(&optval) as libc::socklen_t,
        );
    }

    if ret != 0 {
        let msg = format!(
            "setsockopt error! ret={}, err={}",
            ret,
            async_std::io::Error::last_os_error()
        );
        error!("{}", msg);

        return Err(Box::<dyn Error>::from(msg));
    }

    Ok(())
}

#[cfg(windows)]
pub fn set_socket_keepalive(sock: RawSocket) -> Result<(), Box<dyn Error>> {
    const SOL_SOCKET: i32 = 0xFFFF;
    const SO_KEEPALIVE: i32 = 0x0008;

    let ret;
    unsafe {
        let optval: libc::c_int = 1;
        ret = libc::setsockopt(
            sock as libc::SOCKET,
            SOL_SOCKET,
            SO_KEEPALIVE,
            &optval as *const _ as *const libc::c_char,
            mem::size_of_val(&optval) as libc::c_int,
        );
    }

    if ret != 0 {
        let msg = format!(
            "setsockopt error! ret={}, err={}",
            ret,
            async_std::io::Error::last_os_error()
        );
        error!("{}", msg);

        return Err(Box::<dyn Error>::from(msg));
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn init_udp_socket(_socket: &UdpSocket) -> Result<(), BuckyError> {
    Ok(())
}

#[cfg(windows)]
pub fn init_udp_socket<S: AsRawSocket>(socket: &S) -> Result<(), BuckyError> {
    unsafe {
        // Ignoring UdpSocket's WSAECONNRESET error
        // https://github.com/shadowsocks/shadowsocks-rust/issues/179
        // https://stackoverflow.com/questions/30749423/is-winsock-error-10054-wsaeconnreset-normal-with-udp-to-from-localhost
        //
        // This is because `UdpSocket::recv_from` may return WSAECONNRESET
        // if you called `UdpSocket::send_to` a destination that is not existed (may be closed).
        //
        // It is not an error. Could be ignored completely.
        // We have to ignore it here because it will crash the server.

        let mut bytes_returned: DWORD = 0;
        let mut enable: BOOL = FALSE;
        let handle = socket.as_raw_socket() as SOCKET;

        let ret = WSAIoctl(
            handle,
            SIO_UDP_CONNRESET,
            &mut enable as *mut _ as LPVOID,
            mem::size_of_val(&enable) as DWORD,
            ptr::null_mut(),
            0,
            &mut bytes_returned as *mut _ as LPDWORD,
            ptr::null_mut(),
            None,
        );

        if ret == SOCKET_ERROR {
            use std::io::Error;

            let err_code = WSAGetLastError();
            let err = Error::from_raw_os_error(err_code);

            Err(BuckyError::from(err))
        } else {
            Ok(())
        }
    }
}

// 解析形如 port or host:port的格式
pub fn parse_address(address: &str) -> Result<(String, u16), BuckyError> {
    let parts: Vec<&str> = address.split(':').collect();
    if parts.len() == 1 {
        match parts[0].parse::<u16>() {
            Ok(port) => Ok(("0.0.0.0".to_string(), port)),
            Err(e) => {
                error!("invalid address port, address={}, e={}", address, e);
                Err(BuckyError::from(e))
            }
        }
    } else {
        match parts[1].parse::<u16>() {
            Ok(port) => Ok((parts[0].to_string(), port)),
            Err(e) => {
                error!("invalid address port, address={}, e={}", address, e);
                Err(BuckyError::from(e))
            }
        }
    }
}


pub fn parse_port_from_toml_value(v: &toml::Value) -> BuckyResult<u16> {
    let port: u16;

    if v.is_integer() {
        let v = v.as_integer().unwrap();
        if v >= 65536 {
            let msg = format!("invalid port number range: {}", v);
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }
        port = v as u16;
    } else if v.is_str() {
        let v = v.as_str().unwrap();
        let ret = v.parse::<u16>();
        if let Err(e) = ret {
            error!("invalid port number! e={}", e);

            return Err(BuckyError::from(e));
        }

        port = ret.unwrap();
    } else {
        let msg = format!("invalid port type: {:?}", v);
        error!("{}", msg);

        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
    }

    Ok(port)
}

pub fn is_invalid_ip(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split(".").collect();

    // 过滤169.254.x.x  0.0.0.0
    return (parts[0] == "169" && parts[1] == "254")
        || (parts[0] == "0" && parts[1] == "0" && parts[2] == "0" && parts[3] == "0");
}

// 判断一个ip是不是内网私有ip
pub fn is_private_ip(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split(".").collect();
    return parts[0] == "10"
        || (parts[0] == "172"
            && (parts[1].parse::<u16>().unwrap_or(0) >= 16
                && parts[1].parse::<u16>().unwrap_or(0) <= 31))
        || (parts[0] == "192" && parts[1] == "168");
}

static SYSTEM_HOST_INFO: once_cell::sync::OnceCell<SystemHostInfo> = once_cell::sync::OnceCell::new();

#[derive(Clone)]
pub struct SystemHostInfo {
    pub none_local_ip_v4: Vec<SocketAddr>,
    pub private_ip_v4: Vec<SocketAddr>,
    pub public_ip_v4: Vec<SocketAddr>,
    pub ip_v6: Vec<SocketAddr>,
}

// 外部可以直接设置system_hosts
pub fn bind_system_hosts(info: SystemHostInfo) {
    if let Err(_e) = SYSTEM_HOST_INFO.set(info) {
        error!("bind_system_hosts must be call only once or on startup before stack init!")
    }
}

pub fn get_system_hosts() -> BuckyResult<SystemHostInfo> {
    if SYSTEM_HOST_INFO.get().is_none() {
        let info = init_system_hosts()?;
        bind_system_hosts(info);
    }

    Ok(SYSTEM_HOST_INFO.get().unwrap().clone())
}

fn init_system_hosts() -> BuckyResult<SystemHostInfo> {
    let ret = get_if_addrs();
    if let Err(e) = ret {
        let msg = format!("get_if_addrs error! err={}", e);
        error!("{}", msg);

        return Err(BuckyError::from(msg));
    }

    let mut result = SystemHostInfo {
        none_local_ip_v4: Vec::new(),
        private_ip_v4: Vec::new(),
        public_ip_v4: Vec::new(),
        ip_v6: Vec::new(),
    };

    for iface in ret.unwrap() {
        info!("got iface={:?}", iface);

        let addr = match iface.ip() {
            IpAddr::V4(addr) => addr,
            IpAddr::V6(addr) => {
                let sock_addr = SocketAddrV6::new(addr, 0, 0, iface.scope_id);
                result.ip_v6.push(SocketAddr::V6(sock_addr));
                continue;
            }
        };

        let ip_str = addr.to_string();
        info!("got ip: {} {}", ip_str, iface.description);

        if cfg!(windows) {
            if iface.description.find("VMware").is_some() {
                info!("will ignore as VMware addr: {}", ip_str);
                continue;
            }

            if iface
                .description
                .find("Hyper-V Virtual Ethernet Adapter")
                .is_some()
            {
                info!(
                    "will ignore as Hyper-V Virtual Ethernet Adapter addr: {}",
                    ip_str
                );
                continue;
            }
        }

        if is_invalid_ip(&ip_str) {
            info!("will ignore as invalid addr: {}", ip_str);
            continue;
        }

        let addr = SocketAddr::V4(SocketAddrV4::new(addr, 0));
        result.none_local_ip_v4.push(addr);
        if is_private_ip(&ip_str) {
            result.private_ip_v4.push(addr);
        } else {
            result.public_ip_v4.push(addr);
        }
    }

    Ok(result)
}
