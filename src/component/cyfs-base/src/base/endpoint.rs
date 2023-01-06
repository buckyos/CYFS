pub use std::net::{IpAddr, SocketAddr};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};
use std::str::FromStr;

use crate::codec::{RawDecode, RawEncode, RawEncodePurpose, RawFixedBytes};
use crate::*;
use std::cmp::Ordering;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Protocol {
    Unk = 0,
    Tcp = 1,
    Udp = 2,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EndpointArea {
    Lan, 
    Default, 
    Wan, 
    Mapped
}

#[derive(Copy, Clone, Eq)]
pub struct Endpoint {
    area: EndpointArea,
    protocol: Protocol,
    addr: SocketAddr,
}

impl Endpoint {
    pub fn protocol(&self) -> Protocol {
        self.protocol
    }
    pub fn set_protocol(&mut self, p: Protocol) {
        self.protocol = p
    }

    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn mut_addr(&mut self) -> &mut SocketAddr {
        &mut self.addr
    }

    pub fn is_same_ip_version(&self, other: &Endpoint) -> bool {
        self.addr.is_ipv4() == other.addr.is_ipv4()
    }

    pub fn is_same_ip_addr(&self, other: &Endpoint) -> bool {
        let mut self_ip = self.addr;
        self_ip.set_port(0);
        let mut other_ip = other.addr;
        other_ip.set_port(0);
        self_ip == other_ip
    }

    pub fn default_of(ep: &Endpoint) -> Self {
        match ep.protocol {
            Protocol::Tcp => Self::default_tcp(ep),
            Protocol::Udp => Self::default_udp(ep),
            _ => Self {
                area: EndpointArea::Lan,
                protocol: Protocol::Unk,
                addr: match ep.addr().is_ipv4() {
                    true => SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)),
                    false => SocketAddr::V6(SocketAddrV6::new(
                        Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
                        0,
                        0,
                        0,
                    )),
                },
            },
        }
    }

    pub fn default_tcp(ep: &Endpoint) -> Self {
        Self {
            area: EndpointArea::Lan,
            protocol: Protocol::Tcp,
            addr: match ep.addr().is_ipv4() {
                true => SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)),
                false => SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
                    0,
                    0,
                    0,
                )),
            },
        }
    }

    pub fn default_udp(ep: &Endpoint) -> Self {
        Self {
            area: EndpointArea::Lan,
            protocol: Protocol::Udp,
            addr: match ep.addr().is_ipv4() {
                true => SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)),
                false => SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
                    0,
                    0,
                    0,
                )),
            },
        }
    }

    pub fn is_udp(&self) -> bool {
        self.protocol == Protocol::Udp
    }
    pub fn is_tcp(&self) -> bool {
        self.protocol == Protocol::Tcp
    }
    pub fn is_sys_default(&self) -> bool {
        self.area == EndpointArea::Default
    }
    pub fn is_static_wan(&self) -> bool {
        self.area == EndpointArea::Wan
            || self.area == EndpointArea::Mapped
    }

    pub fn is_mapped_wan(&self) -> bool {
        self.area == EndpointArea::Mapped
    }

    pub fn set_area(&mut self, area: EndpointArea) {
        self.area = area;
    }
}

impl Default for Endpoint {
    fn default() -> Self {
        Self {
            area: EndpointArea::Lan,
            protocol: Protocol::Unk,
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)),
        }
    }
}

impl From<(Protocol, SocketAddr)> for Endpoint {
    fn from(ps: (Protocol, SocketAddr)) -> Self {
        Self {
            area: EndpointArea::Lan,
            protocol: ps.0,
            addr: ps.1,
        }
    }
}

impl From<(Protocol, IpAddr, u16)> for Endpoint {
    fn from(piu: (Protocol, IpAddr, u16)) -> Self {
        Self {
            area: EndpointArea::Lan,
            protocol: piu.0,
            addr: SocketAddr::new(piu.1, piu.2),
        }
    }
}

impl ToSocketAddrs for Endpoint {
    type Iter = <SocketAddr as ToSocketAddrs>::Iter;
    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        self.addr.to_socket_addrs()
    }
}

impl PartialEq for Endpoint {
    fn eq(&self, other: &Endpoint) -> bool {
        self.protocol == other.protocol && self.addr == other.addr
    }
}

impl PartialOrd for Endpoint {
    fn partial_cmp(&self, other: &Endpoint) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        match self.protocol.partial_cmp(&other.protocol).unwrap() {
            Equal => match self.addr.ip().partial_cmp(&other.addr().ip()) {
                None => self.addr.port().partial_cmp(&other.addr.port()),
                Some(ord) => match ord {
                    Greater => Some(Greater),
                    Less => Some(Less),
                    Equal => self.addr.port().partial_cmp(&other.addr.port()),
                },
            },
            Greater => Some(Greater),
            Less => Some(Less),
        }
    }
}

impl Ord for Endpoint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}


impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut result = String::new();

        result += match self.area {
            EndpointArea::Lan => "L", // LOCAL
            EndpointArea::Default => "D", // DEFAULT, 
            EndpointArea::Wan =>  "W", // WAN, 
            EndpointArea::Mapped => "M" // MAPPED WAN, 
        };

        result += match self.addr {
            SocketAddr::V4(_) => "4",
            SocketAddr::V6(_) => "6",
        };

        result += match self.protocol {
            Protocol::Unk => "unk",
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
        };

        result += self.addr.to_string().as_str();

        write!(f, "{}", &result)
    }
}

impl FromStr for Endpoint {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let area = {
            match &s[0..1] {
                "W" => Ok(EndpointArea::Wan),
                "M" => Ok(EndpointArea::Mapped),
                "L" => Ok(EndpointArea::Lan),
                "D" => Ok(EndpointArea::Default),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::InvalidInput,
                    "invalid endpoint string",
                )),
            }
        }?;
        let version_str = &s[1..2];

        let protocol = {
            match &s[2..5] {
                "tcp" => Ok(Protocol::Tcp),
                "udp" => Ok(Protocol::Udp),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::InvalidInput,
                    "invalid endpoint string",
                )),
            }
        }?;

        let addr = SocketAddr::from_str(&s[5..]).map_err(|_| {
            BuckyError::new(BuckyErrorCode::InvalidInput, "invalid endpoint string")
        })?;
        if !(addr.is_ipv4() && version_str.eq("4") || addr.is_ipv6() && version_str.eq("6")) {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidInput,
                "invalid endpoint string",
            ));
        }
        Ok(Endpoint {
            area, 
            protocol,
            addr,
        })
    }
}

pub fn endpoints_to_string(eps: &[Endpoint]) -> String {
    let mut s = "[".to_string();
    if eps.len() > 0 {
        s += eps[0].to_string().as_str();
    }

    if eps.len() > 1 {
        for i in 1..eps.len() {
            s += ",";
            s += eps[i].to_string().as_str();
        }
    }
    s += "]";
    s
}

// 标识默认地址，socket bind的时候用0 地址
const ENDPOINT_FLAG_DEFAULT: u8 = 1u8 << 0;

const ENDPOINT_PROTOCOL_UNK: u8 = 0;
const ENDPOINT_PROTOCOL_TCP: u8 = 1u8 << 1;
const ENDPOINT_PROTOCOL_UDP: u8 = 1u8 << 2;

const ENDPOINT_IP_VERSION_4: u8 = 1u8 << 3;
const ENDPOINT_IP_VERSION_6: u8 = 1u8 << 4;
const ENDPOINT_FLAG_STATIC_WAN: u8 = 1u8 << 6;
const ENDPOINT_FLAG_SIGNED: u8 = 1u8 << 7;

#[derive(Clone)]
pub struct SignedEndpoint(Endpoint);

impl From<Endpoint> for SignedEndpoint {
    fn from(ep: Endpoint) -> Self {
        Self(ep)
    }
}

impl Into<Endpoint> for SignedEndpoint {
    fn into(self) -> Endpoint {
        self.0
    }
}

impl AsRef<Endpoint> for SignedEndpoint {
    fn as_ref(&self) -> &Endpoint {
        &self.0
    }
}

impl RawFixedBytes for Endpoint {
    // TOFIX: C BDT union addr and addrV6 should not memcpy directly
    fn raw_max_bytes() -> Option<usize> {
        Some(1 + 2 + 16)
    }
    fn raw_min_bytes() -> Option<usize> {
        Some(1 + 2 + 4)
    }
}

impl RawFixedBytes for SignedEndpoint {
    // TOFIX: C BDT union addr and addrV6 should not memcpy directly
    fn raw_max_bytes() -> Option<usize> {
        Some(1 + 2 + 16)
    }
    fn raw_min_bytes() -> Option<usize> {
        Some(1 + 2 + 4)
    }
}

impl Endpoint {
    fn flags(&self) -> u8 {
        let mut flags = 0u8;
        flags |= match self.protocol {
            Protocol::Tcp => ENDPOINT_PROTOCOL_TCP,
            Protocol::Unk => ENDPOINT_PROTOCOL_UNK,
            Protocol::Udp => ENDPOINT_PROTOCOL_UDP,
        };
        flags |= match self.is_static_wan() {
            true => ENDPOINT_FLAG_STATIC_WAN,
            false => 0,
        };
        flags |= match self.is_sys_default() {
            true => ENDPOINT_FLAG_DEFAULT,
            false => 0,
        };
        flags |= match self.addr {
            SocketAddr::V4(_) => ENDPOINT_IP_VERSION_4,
            SocketAddr::V6(_) => ENDPOINT_IP_VERSION_6,
        };
        flags
    }

    fn raw_encode_no_flags<'a>(&self, buf: &'a mut [u8]) -> Result<&'a mut [u8], BuckyError> {
        buf[0..2].copy_from_slice(&self.addr.port().to_le_bytes()[..]);
        let buf = &mut buf[2..];

        match self.addr {
            SocketAddr::V4(ref sock_addr) => {
                if buf.len() < 4 {
                    let msg = format!(
                        "not enough buffer for encode SocketAddrV4, except={}, got={}",
                        4,
                        buf.len()
                    );
                    error!("{}", msg);

                    Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg))
                } else {
                    unsafe {
                        std::ptr::copy(
                            sock_addr.ip().octets().as_ptr() as *const u8,
                            buf.as_mut_ptr(),
                            4,
                        );
                    }
                    Ok(&mut buf[4..])
                }
            }
            SocketAddr::V6(ref sock_addr) => {
                if buf.len() < 16 {
                    let msg = format!(
                        "not enough buffer for encode SocketAddrV6, except={}, got={}",
                        16,
                        buf.len()
                    );
                    error!("{}", msg);

                    Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg))
                } else {
                    buf[..16].copy_from_slice(&sock_addr.ip().octets());
                    Ok(&mut buf[16..])
                }
            }
        }
    }

    fn raw_decode_no_flags<'de>(
        flags: u8,
        buf: &'de [u8],
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let protocol = match flags & ENDPOINT_PROTOCOL_TCP {
            0 => match flags & ENDPOINT_PROTOCOL_UDP {
                0 => Protocol::Unk,
                _ => Protocol::Udp,
            },
            _ => Protocol::Tcp,
        };

        let area = if flags & ENDPOINT_FLAG_STATIC_WAN != 0 {
            EndpointArea::Wan
        } else if flags & ENDPOINT_FLAG_DEFAULT != 0 {
            EndpointArea::Default
        } else {
            EndpointArea::Lan
        };
       

        let port = {
            let mut b = [0u8; 2];
            b.copy_from_slice(&buf[0..2]);
            u16::from_le_bytes(b)
        };
        let buf = &buf[2..];

        let (addr, buf) = {
            if flags & ENDPOINT_IP_VERSION_6 != 0 {
                if buf.len() < 16 {
                    let msg = format!(
                        "not enough buffer for decode EndPoint6, except={}, got={}",
                        16,
                        buf.len()
                    );
                    error!("{}", msg);

                    Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg))
                } else {
                    let mut s: [u8; 16] = [0; 16];
                    s.copy_from_slice(&buf[..16]);
                    // TOFIX: flow and scope
                    let addr = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(s), port, 0, 0));
                    Ok((addr, &buf[16..]))
                }
            } else {
                let addr = SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]),
                    port,
                ));
                Ok((addr, &buf[4..]))
            }
        }?;

        let ep = Endpoint {
            area, 
            protocol,
            addr,
        };
        Ok((ep, buf))
    }
}

impl RawEncode for Endpoint {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        match self.addr {
            SocketAddr::V4(_) => Ok(1 + 2 + 4),
            SocketAddr::V6(_) => Ok(1 + 2 + 16),
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let min_bytes = Self::raw_min_bytes().unwrap();
        if buf.len() < min_bytes {
            let msg = format!(
                "not enough buffer for encode Endpoint, min bytes={}, got={}",
                min_bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        buf[0] = self.flags();
        self.raw_encode_no_flags(&mut buf[1..])
    }
}

impl<'de> RawDecode<'de> for Endpoint {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let min_bytes = Self::raw_min_bytes().unwrap();
        if buf.len() < min_bytes {
            let msg = format!(
                "not enough buffer for decode Endpoint, min bytes={}, got={}",
                min_bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        let flags = buf[0];
        Self::raw_decode_no_flags(flags, &buf[1..])
    }
}

impl RawEncode for SignedEndpoint {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        self.0.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = self.raw_measure(purpose)?;
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode SignedEndpoint, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        buf[0] = self.0.flags() | ENDPOINT_FLAG_SIGNED;
        self.0.raw_encode_no_flags(&mut buf[1..])
    }
}

impl<'de> RawDecode<'de> for SignedEndpoint {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let min_bytes = Self::raw_min_bytes().unwrap();
        if buf.len() < min_bytes {
            let msg = format!(
                "not enough buffer for decode SignedEndpoint, min bytes={}, got={}",
                min_bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        let flags = buf[0];
        if flags & ENDPOINT_FLAG_SIGNED == 0 {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidParam,
                "without sign flag",
            ));
        }
        let (ep, buf) = Endpoint::raw_decode_no_flags(flags, &buf[1..])?;
        Ok((SignedEndpoint(ep), buf))
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use async_std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
    use std::convert::From;
    //use std::path::Path;

    #[test]
    fn test_codec() {
        let ep = Endpoint::default();
        let v = ep.to_vec().unwrap();
        let ep2 = Endpoint::clone_from_slice(&v).unwrap();
        assert_eq!(ep, ep2);

        let ep: Endpoint = (
            Protocol::Tcp,
            SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(127, 11, 22, 33), 4)),
        )
            .into();
        let v = ep.to_vec().unwrap();
        let ep2 = Endpoint::clone_from_slice(&v).unwrap();
        assert_eq!(ep, ep2);
    }
    #[test]
    fn endpoint() {
        let ep: Endpoint = (
            Protocol::Tcp,
            SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(127, 1, 2, 3), 4)),
        )
            .into();
        println!("{}", ep);

        // let p = Path::new("f:\\temp\\endpoint.obj");
        // if p.parent().unwrap().exists() {
        //     ep.encode_to_file(p, false);
        // }

        let ep: Endpoint = (
            Protocol::Tcp,
            SocketAddr::from(SocketAddrV6::new(
                Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8),
                9,
                10,
                11,
            )),
        )
            .into();
        println!("{}", ep);
        // let p = Path::new("f:\\temp\\endpoint2.obj");
        // if p.parent().unwrap().exists() {
        //     ep.encode_to_file(p, false);
        // }
    }
}
