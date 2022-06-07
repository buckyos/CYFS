//! get_if_addrs
#[cfg(windows)]
extern crate winapi;

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[cfg(not(windows))]
#[cfg(windows)]
use std::os::windows::prelude::*;

extern crate c_linked_list;
#[cfg(not(windows))]
extern crate libc;

/// Details about an interface on this host
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Interface {
    /// The name of the interface.
    pub name: String,
    /// The address details of the interface.
    pub addr: IfAddr,

    pub description: String,

    pub ifa_flags: u32,

    pub scope_id: u32,
}

/// Details about the address of an interface on this host
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum IfAddr {
    /// This is an Ipv4 interface.
    V4(Ifv4Addr),
    /// This is an Ipv6 interface.
    V6(Ifv6Addr),
}

/// Details about the ipv4 address of an interface on this host
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Ifv4Addr {
    /// The IP address of the interface.
    pub ip: Ipv4Addr,
    /// The netmask of the interface.
    pub netmask: Ipv4Addr,
    /// The broadcast address of the interface.
    pub broadcast: Option<Ipv4Addr>,
}

/// Details about the ipv6 address of an interface on this host
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Ifv6Addr {
    /// The IP address of the interface.
    pub ip: Ipv6Addr,
    /// The netmask of the interface.
    pub netmask: Ipv6Addr,
    /// The broadcast address of the interface.
    pub broadcast: Option<Ipv6Addr>,
}

impl Interface {
    /// Check whether this is a loopback interface.
    pub fn is_loopback(&self) -> bool {
        self.addr.is_loopback()
    }

    /// Get the IP address of this interface.
    pub fn ip(&self) -> IpAddr {
        self.addr.ip()
    }
}

impl IfAddr {
    /// Check whether this is a loopback address.
    pub fn is_loopback(&self) -> bool {
        match *self {
            IfAddr::V4(ref ifv4_addr) => ifv4_addr.is_loopback(),
            IfAddr::V6(ref ifv6_addr) => ifv6_addr.is_loopback(),
        }
    }

    /// Get the IP address of this interface address.
    pub fn ip(&self) -> IpAddr {
        match *self {
            IfAddr::V4(ref ifv4_addr) => IpAddr::V4(ifv4_addr.ip),
            IfAddr::V6(ref ifv6_addr) => IpAddr::V6(ifv6_addr.ip),
        }
    }
}

impl Ifv4Addr {
    /// Check whether this is a loopback address.
    pub fn is_loopback(&self) -> bool {
        self.ip.octets()[0] == 127
    }
}

impl Ifv6Addr {
    /// Check whether this is a loopback address.
    pub fn is_loopback(&self) -> bool {
        self.ip.segments() == [0, 0, 0, 0, 0, 0, 0, 1]
    }
}

#[cfg(not(windows))]
mod getifaddrs_posix {
    use super::{IfAddr, Ifv4Addr, Ifv6Addr, Interface};
    use c_linked_list::CLinkedListMut;
    use libc::freeifaddrs as posix_freeifaddrs;
    use libc::getifaddrs as posix_getifaddrs;
    use libc::ifaddrs as posix_ifaddrs;

    use libc::sockaddr as posix_sockaddr;
    use libc::sockaddr_in as posix_sockaddr_in;
    use libc::sockaddr_in6 as posix_sockaddr_in6;
    use libc::{AF_INET, AF_INET6};
    use std::ffi::CStr;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::{io, mem};

    #[allow(non_camel_case_types)]
    pub enum IFFFlags {
        IFF_UP = 1 << 0,          /* sysfs */
        IFF_BROADCAST = 1 << 1,   /* volatile */
        IFF_DEBUG = 1 << 2,       /* sysfs */
        IFF_LOOPBACK = 1 << 3,    /* volatile */
        IFF_POINTOPOINT = 1 << 4, /* volatile */
        IFF_NOTRAILERS = 1 << 5,  /* sysfs */
        IFF_RUNNING = 1 << 6,     /* volatile */
        IFF_NOARP = 1 << 7,       /* sysfs */
        IFF_PROMISC = 1 << 8,     /* sysfs */
        IFF_ALLMULTI = 1 << 9,    /* sysfs */
        IFF_MASTER = 1 << 10,     /* volatile */
        IFF_SLAVE = 1 << 11,      /* volatile */
        IFF_MULTICAST = 1 << 12,  /* sysfs */
        IFF_PORTSEL = 1 << 13,    /* sysfs */
        IFF_AUTOMEDIA = 1 << 14,  /* sysfs */
        IFF_DYNAMIC = 1 << 15,    /* sysfs */
        IFF_LOWER_UP = 1 << 16,   /* volatile */
        IFF_DORMANT = 1 << 17,    /* volatile */
        IFF_ECHO = 1 << 18,       /* volatile */
    }

    #[allow(unsafe_code)]
    fn sockaddr_to_ipaddr(sockaddr: *const posix_sockaddr) -> Option<(IpAddr, u32)> {
        if sockaddr.is_null() {
            return None;
        }

        let sa_family = u32::from(unsafe { *sockaddr }.sa_family);

        if sa_family == AF_INET as u32 {
            #[cfg_attr(feature = "cargo-clippy", allow(cast_ptr_alignment))]
            let sa = &unsafe { *(sockaddr as *const posix_sockaddr_in) };
            Some((IpAddr::V4(Ipv4Addr::new(
                ((sa.sin_addr.s_addr) & 255) as u8,
                ((sa.sin_addr.s_addr >> 8) & 255) as u8,
                ((sa.sin_addr.s_addr >> 16) & 255) as u8,
                ((sa.sin_addr.s_addr >> 24) & 255) as u8,
            )), 0))
        } else if sa_family == AF_INET6 as u32 {
            #[cfg_attr(feature = "cargo-clippy", allow(cast_ptr_alignment))]
            let sa = &unsafe { *(sockaddr as *const posix_sockaddr_in6) };
            // Ignore all fe80:: addresses as these are link locals
            /*
            if sa.sin6_addr.s6_addr[0] != 0xfe || sa.sin6_addr.s6_addr[1] != 0x80 {
                return None;
            }
            */
            Some((IpAddr::V6(Ipv6Addr::from(sa.sin6_addr.s6_addr)), sa.sin6_scope_id))
        } else {
            None
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "nacl"))]
    fn do_broadcast(ifaddr: &posix_ifaddrs) -> Option<(IpAddr, u32)> {
        sockaddr_to_ipaddr(ifaddr.ifa_ifu)
    }

    #[cfg(any(
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "openbsd"
    ))]
    fn do_broadcast(ifaddr: &posix_ifaddrs) -> Option<(IpAddr, u32)> {
        sockaddr_to_ipaddr(ifaddr.ifa_addr)
    }

    /// Return a vector of IP details for all the valid interfaces on this host
    #[allow(unsafe_code)]
    #[allow(trivial_casts)]
    pub fn get_if_addrs() -> io::Result<Vec<Interface>> {
        let mut ret = Vec::<Interface>::new();
        let mut ifaddrs: *mut posix_ifaddrs;
        unsafe {
            ifaddrs = mem::MaybeUninit::uninit().assume_init();
            if -1 == posix_getifaddrs(&mut ifaddrs) {
                return Err(io::Error::last_os_error());
            }
        }

        for ifaddr in unsafe { CLinkedListMut::from_ptr(ifaddrs, |a| a.ifa_next) }.iter() {
            if ifaddr.ifa_addr.is_null() {
                continue;
            }

            let name = unsafe { CStr::from_ptr(ifaddr.ifa_name as *const _) }
                .to_string_lossy()
                .into_owned();

            // 过滤掉状态不为up和一些虚拟网卡
            if ifaddr.ifa_flags & (IFFFlags::IFF_UP as u32) == 0 {
                info!("will ignore iface {}", name);
                continue;
            }
    
            if (ifaddr.ifa_flags & (IFFFlags::IFF_LOOPBACK as u32) != 0)
                || (ifaddr.ifa_flags & (IFFFlags::IFF_POINTOPOINT as u32) != 0)
            {
                info!("will ignore iface {}", name);
                continue;
            }


            let (addr, scope_id) = match sockaddr_to_ipaddr(ifaddr.ifa_addr) {
                None => continue,
                Some((IpAddr::V4(ipv4_addr), _)) => {
                    if ipv4_addr.is_loopback()
                        || ipv4_addr.is_unspecified()
                        || ipv4_addr.is_link_local()
                    {
                        continue;
                    }

                    let netmask = match sockaddr_to_ipaddr(ifaddr.ifa_netmask) {
                        Some((IpAddr::V4(netmask), _)) => netmask,
                        _ => Ipv4Addr::new(0, 0, 0, 0),
                    };

                    let broadcast = if (ifaddr.ifa_flags & IFFFlags::IFF_BROADCAST as u32) != 0 {
                        match do_broadcast(ifaddr) {
                            Some((IpAddr::V4(broadcast), _)) => Some(broadcast),
                            _ => None,
                        }
                    } else {
                        None
                    };

                    (IfAddr::V4(Ifv4Addr {
                        ip: ipv4_addr,
                        netmask,
                        broadcast,
                    }), 0)
                }
                Some((IpAddr::V6(ipv6_addr), scope_id)) => {
                    if ipv6_addr.is_loopback() || ipv6_addr.is_unspecified() {
                        continue;
                    }

                    let netmask = match sockaddr_to_ipaddr(ifaddr.ifa_netmask) {
                        Some((IpAddr::V6(netmask), _)) => netmask,
                        _ => Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
                    };

                    // if (ifaddr.ifa_flags & 0x01)==0x01 {
                    //     continue;
                    // }

                    let broadcast = if (ifaddr.ifa_flags & IFFFlags::IFF_BROADCAST as u32) != 0 {
                        match do_broadcast(ifaddr) {
                            Some((IpAddr::V6(broadcast), _)) => Some(broadcast),
                            _ => None,
                        }
                    } else {
                        None
                    };

                    (IfAddr::V6(Ifv6Addr {
                        ip: ipv6_addr,
                        netmask,
                        broadcast,
                    }), scope_id)
                }
            };

            ret.push(Interface {
                name,
                addr,
                description: String::from(""),
                ifa_flags: ifaddr.ifa_flags as u32,
                scope_id
            });
        }
        unsafe {
            posix_freeifaddrs(ifaddrs);
        }
        Ok(ret)
    }
}

/// Get a list of all the network interfaces on this machine along with their IP info.
#[cfg(not(windows))]
pub fn get_if_addrs() -> io::Result<Vec<Interface>> {
    getifaddrs_posix::get_if_addrs()
}

#[cfg(not(windows))]
pub use getifaddrs_posix::IFFFlags;

#[cfg(windows)]
mod getifaddrs_windows {
    use super::{IfAddr, Ifv4Addr, Ifv6Addr, Interface};
    use c_linked_list::CLinkedListConst;
    use libc;
    use libc::{c_char, c_int, c_ulong, size_t};
    use std::ffi::{c_void, CStr, OsString};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::os::windows::prelude::*;
    use std::{io, ptr};
    use winapi::shared::minwindef::DWORD;
    use winapi::shared::winerror::ERROR_SUCCESS;
    use winapi::shared::ws2def::SOCKADDR as sockaddr;
    use winapi::shared::ws2def::SOCKADDR_IN as sockaddr_in;
    use winapi::shared::ws2def::{AF_INET, AF_INET6};
    use winapi::shared::ws2ipdef::SOCKADDR_IN6_LH as sockaddr_in6;

    #[repr(C)]
    struct SocketAddress {
        pub lp_socket_address: *const sockaddr,
        pub i_socket_address_length: c_int,
    }
    #[repr(C)]
    struct IpAdapterUnicastAddress {
        pub length: c_ulong,
        pub flags: DWORD,
        pub next: *const IpAdapterUnicastAddress,
        // Loads more follows, but I'm not bothering to map these for now
        pub address: SocketAddress,
        pub prefix_origin: c_ulong,
        pub suffix_origin: c_ulong,
    }
    #[repr(C)]
    struct IpAdapterPrefix {
        pub length: c_ulong,
        pub flags: DWORD,
        pub next: *const IpAdapterPrefix,
        pub address: SocketAddress,
        pub prefix_length: c_ulong,
    }
    #[repr(C)]
    struct IpAdapterAddresses {
        pub length: c_ulong,
        pub if_index: DWORD,
        pub next: *const IpAdapterAddresses,
        pub adapter_name: *const c_char,
        pub first_unicast_address: *const IpAdapterUnicastAddress,
        first_anycast_address: *const c_void,
        first_multicast_address: *const c_void,
        first_dns_server_address: *const c_void,
        dns_suffix: *const c_void,
        description: *const c_void,
        friendly_name: *const c_void,
        physical_address: [c_char; 8],
        physical_address_length: DWORD,
        flags: DWORD,
        mtu: DWORD,
        if_type: DWORD,
        oper_status: c_int,
        ipv6_if_index: DWORD,
        zone_indices: [DWORD; 16],
        // Loads more follows, but I'm not bothering to map these for now
        pub first_prefix: *const IpAdapterPrefix,
    }
    #[link(name = "Iphlpapi")]
    extern "system" {
        /// get adapter's addresses
        fn GetAdaptersAddresses(
            family: c_ulong,
            flags: c_ulong,
            reserved: *const c_void,
            addresses: *const IpAdapterAddresses,
            size: *mut c_ulong,
        ) -> c_ulong;
    }

    #[allow(unsafe_code)]
    fn sockaddr_to_ipaddr(sockaddr: *const sockaddr) -> Option<(IpAddr, u32)> {
        if sockaddr.is_null() {
            return None;
        }
        if unsafe { *sockaddr }.sa_family as u32 == AF_INET as u32 {
            let sa = &unsafe { *(sockaddr as *const sockaddr_in) };
            // Ignore all 169.254.x.x addresses as these are not active interfaces

            if unsafe { sa.sin_addr.S_un.S_un_w().s_w1 } == 0xa9fe {
                return None;
            }

            Some((IpAddr::V4(Ipv4Addr::new(
                unsafe { sa.sin_addr.S_un.S_un_b().s_b1 },
                unsafe { sa.sin_addr.S_un.S_un_b().s_b2 },
                unsafe { sa.sin_addr.S_un.S_un_b().s_b3 },
                unsafe { sa.sin_addr.S_un.S_un_b().s_b4 },
            )), 0))
        } else if unsafe { *sockaddr }.sa_family as u32 == AF_INET6 as u32 {
            let sa = &unsafe { *(sockaddr as *const sockaddr_in6) };
            // Ignore all fe80:: addresses as these are link locals
            /*
            unsafe {
                if sa.sin6_addr.u.Word()[0] != 0x80fe {
                    return None;
                }
            }
             */

            let mut v6byte = [0_u8; 16];
            v6byte.copy_from_slice(unsafe { sa.sin6_addr.u.Byte() });
            Some((IpAddr::V6(Ipv6Addr::from(v6byte)), *unsafe { sa.u.sin6_scope_id() }))
        } else {
            None
        }
    }

    unsafe fn u16_ptr_to_string(ptr: *const u16) -> OsString {
        let len = (0..).take_while(|&i| *ptr.offset(i) != 0).count();
        let slice = std::slice::from_raw_parts(ptr, len);

        OsString::from_wide(slice)
    }

    // trivial_numeric_casts lint may become allow by default.
    // Refer: https://github.com/rust-lang/rfcs/issues/1020
    /// Return a vector of IP details for all the valid interfaces on this host
    #[allow(unsafe_code, trivial_numeric_casts)]
    pub fn get_if_addrs() -> io::Result<Vec<Interface>> {
        let mut ret = Vec::<Interface>::new();
        let mut ifaddrs: *const IpAdapterAddresses;
        let mut buffersize: c_ulong = 15000;
        loop {
            unsafe {
                ifaddrs = libc::malloc(buffersize as size_t) as *mut IpAdapterAddresses;
                if ifaddrs.is_null() {
                    panic!("Failed to allocate buffer in get_if_addrs()");
                }
                let retcode = GetAdaptersAddresses(
                    0,
                    // GAA_FLAG_SKIP_ANYCAST       |
                    // GAA_FLAG_SKIP_MULTICAST     |
                    // GAA_FLAG_SKIP_DNS_SERVER    |
                    // GAA_FLAG_INCLUDE_PREFIX     |
                    // GAA_FLAG_SKIP_FRIENDLY_NAME
                    0x3e,
                    ptr::null(),
                    ifaddrs,
                    &mut buffersize,
                );
                match retcode {
                    ERROR_SUCCESS => break,
                    111 => {
                        libc::free(ifaddrs as *mut c_void);
                        buffersize *= 2;
                        continue;
                    }
                    _ => return Err(io::Error::last_os_error()),
                }
            }
        }

        for ifaddr in unsafe { CLinkedListConst::from_ptr(ifaddrs, |a| a.next) }.iter() {
            if ifaddr.oper_status != 1 {
                continue;
            }
            if ifaddr.if_type == 24 || ifaddr.if_type == 131 {
                continue;
            }
            for addr in
                unsafe { CLinkedListConst::from_ptr(ifaddr.first_unicast_address, |a| a.next) }
                    .iter()
            {
                let name = unsafe { CStr::from_ptr(ifaddr.adapter_name) }
                    .to_string_lossy()
                    .into_owned();

                // 过滤docker网卡
                if name.starts_with("docker") {
                    info!("will ignore as Docker Virtual Ethernet Adapter: {}", name);
                    continue;
                }

                let description = unsafe { u16_ptr_to_string(ifaddr.description as *const u16) }
                    .to_string_lossy()
                    .into_owned();

                // 过滤hyper-v和vmware虚拟网卡
                /*
                if description.find("Hyper-V Virtual Ethernet Adapter").is_some() {
                    info!("will ignore as Hyper-V Virtual Ethernet Adapter addr: {}", description);
                    continue;
                }
                */

                if description.find("VMware").is_some() {
                    info!("will ignore as VMware addr: {}", description);
                    continue;
                }

                let (addr, scope_id) = match sockaddr_to_ipaddr(addr.address.lp_socket_address) {
                    None => continue,
                    Some((IpAddr::V4(ipv4_addr), _)) => {

                        if ipv4_addr.is_loopback() 
                            || ipv4_addr.is_link_local()
                            || ipv4_addr.is_broadcast()
                            || ipv4_addr.is_documentation()
                            || ipv4_addr.is_unspecified()
                            // || ipv4_addr.is_reserved()
                        {
                            info!("will ignore ip addr: desc={}, addr={}", description, ipv4_addr);
                            continue;
                        }

                        let mut item_netmask = Ipv4Addr::new(0, 0, 0, 0);
                        let mut item_broadcast = None;
                        // Search prefixes for a prefix matching addr
                        'prefixloopv4: for prefix in
                            unsafe { CLinkedListConst::from_ptr(ifaddr.first_prefix, |p| p.next) }
                                .iter()
                        {
                            let ipprefix = sockaddr_to_ipaddr(prefix.address.lp_socket_address);
                            match ipprefix {
                                Some((IpAddr::V4(ref a), _)) => {
                                    let mut netmask: [u8; 4] = [0; 4];
                                    for (n, netmask_elt) in netmask
                                        .iter_mut()
                                        .enumerate()
                                        .take((prefix.prefix_length as usize + 7) / 8)
                                    {
                                        let x_byte = ipv4_addr.octets()[n];
                                        let y_byte = a.octets()[n];
                                        // Clippy 0.0.128 doesn't handle the label on the `continue`
                                        #[cfg_attr(
                                            feature = "cargo-clippy",
                                            allow(needless_continue)
                                        )]
                                        for m in 0..8 {
                                            if (n * 8) + m > prefix.prefix_length as usize {
                                                break;
                                            }
                                            let bit = 1_u8 << m as u8;
                                            if (x_byte & bit) == (y_byte & bit) {
                                                *netmask_elt |= bit;
                                            } else {
                                                continue 'prefixloopv4;
                                            }
                                        }
                                    }
                                    item_netmask = Ipv4Addr::new(
                                        netmask[0], netmask[1], netmask[2], netmask[3],
                                    );
                                    let mut broadcast: [u8; 4] = ipv4_addr.octets();
                                    for n in 0..4 {
                                        broadcast[n] |= !netmask[n];
                                    }
                                    item_broadcast = Some(Ipv4Addr::new(
                                        broadcast[0],
                                        broadcast[1],
                                        broadcast[2],
                                        broadcast[3],
                                    ));
                                    break 'prefixloopv4;
                                }
                                _ => continue,
                            };
                        }
                        (IfAddr::V4(Ifv4Addr {
                            ip: ipv4_addr,
                            netmask: item_netmask,
                            broadcast: item_broadcast,
                        }), 0)
                    }
                    Some((IpAddr::V6(ipv6_addr), scope_id)) => {
                        let mut item_netmask = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0);
                        // Search prefixes for a prefix matching addr
                        'prefixloopv6: for prefix in
                            unsafe { CLinkedListConst::from_ptr(ifaddr.first_prefix, |p| p.next) }
                                .iter()
                        {
                            let ipprefix = sockaddr_to_ipaddr(prefix.address.lp_socket_address);
                            match ipprefix {
                                Some((IpAddr::V6(ref a), _)) => {
                                    // Iterate the bits in the prefix, if they all match this prefix
                                    // is the right one, else try the next prefix
                                    let mut netmask: [u16; 8] = [0; 8];
                                    for (n, netmask_elt) in netmask
                                        .iter_mut()
                                        .enumerate()
                                        .take((prefix.prefix_length as usize + 15) / 16)
                                    {
                                        let x_word = ipv6_addr.segments()[n];
                                        let y_word = a.segments()[n];
                                        // Clippy 0.0.128 doesn't handle the label on the `continue`
                                        #[cfg_attr(
                                            feature = "cargo-clippy",
                                            allow(needless_continue)
                                        )]
                                        for m in 0..16 {
                                            if (n * 16) + m > prefix.prefix_length as usize {
                                                break;
                                            }
                                            let bit = 1_u16 << m as u16;
                                            if (x_word & bit) == (y_word & bit) {
                                                *netmask_elt |= bit;
                                            } else {
                                                continue 'prefixloopv6;
                                            }
                                        }
                                    }
                                    item_netmask = Ipv6Addr::new(
                                        netmask[0], netmask[1], netmask[2], netmask[3], netmask[4],
                                        netmask[5], netmask[6], netmask[7],
                                    );
                                    break 'prefixloopv6;
                                }
                                _ => continue,
                            };
                        }
                        (IfAddr::V6(Ifv6Addr {
                            ip: ipv6_addr,
                            netmask: item_netmask,
                            broadcast: None,
                        }), scope_id)
                    }
                };
                ret.push(Interface {
                    name,
                    addr,
                    description,
                    ifa_flags: ifaddr.flags as u32,
                    scope_id
                });
            }
        }
        unsafe {
            libc::free(ifaddrs as *mut c_void);
        }
        Ok(ret)
    }
}

#[cfg(windows)]
/// Get address
pub fn get_if_addrs() -> io::Result<Vec<Interface>> {
    getifaddrs_windows::get_if_addrs()
}

#[test]
fn test() {
    let interfaces = get_if_addrs().unwrap();
    for interface in interfaces {
        let addr_str = match interface.addr {
            IfAddr::V4(ip) => {ip.ip.to_string()}
            IfAddr::V6(ip) => {ip.ip.to_string()}
        };
        println!("{}: {}", addr_str, interface.scope_id);
    }
}
