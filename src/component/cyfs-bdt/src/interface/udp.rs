use crate::{
    history::keystore,
    protocol::*,
    stack::{Stack, WeakStack}
};
use super::{
    manager::UpdateOuterResult
};
use async_std::sync::Arc;
use cyfs_base::*;
use log::*;
use std::{cell::RefCell, net::UdpSocket, sync::RwLock, thread};
use socket2::{Socket, Domain, Type};

#[derive(Clone)]
pub struct Config {
    pub sim_loss_rate: u8, 
    pub recv_buffer: usize, 
    pub sn_only: bool
}

pub struct UdpPackageBox {
    package_box: PackageBox,
    remote: Endpoint,
    local: Interface,
}

impl UdpPackageBox {
    pub fn new(package_box: PackageBox, local: Interface, remote: Endpoint) -> Self {
        Self {
            package_box,
            local,
            remote,
        }
    }

    pub fn remote(&self) -> &Endpoint {
        &self.remote
    }
    pub fn local(&self) -> &Interface {
        &self.local
    }
}

impl Into<PackageBox> for UdpPackageBox {
    fn into(self) -> PackageBox {
        self.package_box
    }
}

impl AsRef<PackageBox> for UdpPackageBox {
    fn as_ref(&self) -> &PackageBox {
        &self.package_box
    }
}

//const MAGIC_NUMBER: u16 = u16::from_be_bytes([0u8, 0x80u8]);

pub trait OnUdpPackageBox {
    fn on_udp_package_box(&self, package_box: UdpPackageBox) -> Result<(), BuckyError>;
}

pub trait OnUdpRawData<Context> {
    fn on_udp_raw_data(&self, data: &[u8], context: Context) -> Result<(), BuckyError>;
}

pub const MTU: usize = 1472;

thread_local! {
    static UDP_RECV_BUFFER: RefCell<[u8; MTU]> = RefCell::new([0u8; MTU]);
    static BOX_CRYPTO_BUFFER: RefCell<[u8; MTU]> = RefCell::new([0u8; MTU]);
}

struct InterfaceImpl {
    config: Config, 
    socket: UdpSocket,
    local: RwLock<Endpoint>,
    outer: RwLock<Option<Endpoint>>,
}

#[derive(Clone)]
pub struct Interface(Arc<InterfaceImpl>);

impl std::fmt::Display for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UdpInterface {{local:{}}}", self.local())
    }
}


impl Interface {
    pub fn bind(local: &Endpoint, config: Config) -> Result<Self, BuckyError> {
        fn bind_socket(bind_addr: &Endpoint, recv_buffer: usize) -> Result<UdpSocket, BuckyError> {
            let domain = if bind_addr.addr().is_ipv6() {
                Domain::IPV6
            } else {
                Domain::IPV4
            };

            let socket = Socket::new(domain, Type::DGRAM, None).unwrap();

            let _ = socket.set_recv_buffer_size(recv_buffer);
            match socket.bind(&bind_addr.addr().clone().into()) {
                Ok(_) => Ok(socket.into()), 
                Err(err) => Err(BuckyError::from(err))
            }
        }
 
        let socket = {
            if local.addr().is_ipv6() {
                #[cfg(windows)]
                {
                    let mut default_local = Endpoint::default_udp(local);
                    default_local.mut_addr().set_port(local.addr().port());
                    match bind_socket(&default_local, config.recv_buffer) {
                        Ok(socket) => {
                            // ??????udp?????????reset
                            cyfs_util::init_udp_socket(&socket).map(|_| socket)
                        }
                        Err(err) => Err(BuckyError::from(err)),
                    }
                }
                #[cfg(not(windows))]
                {
                    use std::os::unix::io::FromRawFd;
                    unsafe {
                        let raw_sock = libc::socket(libc::AF_INET6, libc::SOCK_DGRAM, 0);
                        let yes: libc::c_int = 1;
                        libc::setsockopt(
                            raw_sock,
                            libc::IPPROTO_IPV6,
                            libc::IPV6_V6ONLY,
                            &yes as *const libc::c_int as *const libc::c_void,
                            std::mem::size_of::<libc::c_int>().try_into().unwrap(),
                        );
                        libc::setsockopt(
                            raw_sock,
                            libc::SOL_SOCKET,
                            libc::SO_REUSEADDR | libc::SO_BROADCAST,
                            &yes as *const libc::c_int as *const libc::c_void,
                            std::mem::size_of::<libc::c_int>().try_into().unwrap(),
                        );

                        let recv_buf: libc::c_int = config.recv_buffer as libc::c_int;                        
                        libc::setsockopt(raw_sock, 
                            libc::SOL_SOCKET, 
                            libc::SO_RCVBUF, 
                            &recv_buf as *const libc::c_int as *const libc::c_void,
                            std::mem::size_of::<libc::c_int>().try_into().unwrap(),);

                        let addr = libc::sockaddr_in6 {
                            #[cfg(any(target_os = "macos",target_os = "ios"))]
                            sin6_len: 24,
                            sin6_family: libc::AF_INET6 as libc::sa_family_t,
                            sin6_port: local.addr().port().to_be(),
                            sin6_flowinfo: 0,
                            sin6_addr: libc::in6_addr { s6_addr: [0u8; 16] },
                            sin6_scope_id: 0,
                        };
                        if libc::bind(
                            raw_sock,
                            &addr as *const libc::sockaddr_in6 as *const libc::sockaddr,
                            std::mem::size_of::<libc::sockaddr_in6>()
                                .try_into()
                                .unwrap(),
                        ) < 0
                        {
                            Err(BuckyError::from((
                                BuckyErrorCode::AlreadyExists,
                                "bind port failed",
                            )))
                        } else {
                            Ok(UdpSocket::from_raw_fd(raw_sock))
                        }
                    }
                }
            } else {
                let bind_addr = {
                    if local.is_sys_default() {
                        let mut default_local = Endpoint::default_udp(local);
                        default_local.mut_addr().set_port(local.addr().port());
        
                        default_local
                    } else {
                        *local
                    }
                };

                bind_socket(&bind_addr, config.recv_buffer)
            }
        }?;

        Ok(Self(Arc::new(InterfaceImpl {
            config, 
            local: RwLock::new(local.clone()),
            socket,
            outer: RwLock::new(None),
        })))
    }

    pub fn reset(&self, local: &Endpoint) -> Self {
        info!("{} reset with {}", self, local);
        *self.0.local.write().unwrap() = local.clone();
        *self.0.outer.write().unwrap() = None;
        self.clone()
    }

    pub fn start(&self, stack: WeakStack) {
        let ci = self.clone();
        thread::spawn(move || {
            ci.recv_loop(stack);
        });
    }

    pub fn close(&self) {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawSocket;
            use winapi::um::winsock2::closesocket;
            unsafe {
                let raw = self.0.socket.as_raw_socket();
                closesocket(raw.try_into().unwrap());
            }
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::io::AsRawFd;
            unsafe {
                let raw = self.0.socket.as_raw_fd();
                libc::close(raw);
            }
        }
    }

    pub fn local(&self) -> Endpoint {
        *self.0.local.read().unwrap()
    }

    pub fn outer(&self) -> Option<Endpoint> {
        *self.0.outer.read().unwrap()
    }

    pub fn update_outer(&self, outer: &Endpoint) -> UpdateOuterResult {
        let self_outer = &mut *self.0.outer.write().unwrap();
        if let Some(outer_ep) = self_outer.as_ref() {
            if *outer_ep != *outer {
                info!("{} reset outer to {}", self, outer);
                *self_outer = Some(*outer);
                UpdateOuterResult::Reset
            } else {
                trace!("{} ignore update outer to {}", self, outer);
                UpdateOuterResult::None
            }
        } else {
            info!("{} update outer to {}", self, outer);
            *self_outer = Some(*outer);
            UpdateOuterResult::Update
        }
    }

    pub fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn on_recv(&self, stack: Stack, recv: &mut [u8], from: Endpoint) {
        if recv[0] & 0x80 != 0 {
            match KeyMixHash::raw_decode(recv) {
                Ok((mut mix_hash, raw_data)) => {
                    mix_hash.as_mut()[0] &= 0x7f;
                    if let Some(found_key) =
                        stack.keystore().get_key_by_mix_hash(&mix_hash, true, true) {
                        if self.0.config.sn_only {
                            return; 
                        }
                        let _ =
                            stack.on_udp_raw_data(raw_data, (self.clone(), found_key.peerid, found_key.aes_key, from));
                        return;
                    }
                }
                Err(err) => {
                    error!("{} decode failed, from={}, len={}, e={}", self, from, recv.len(), &err);
                    return;
                }
            }
        }
        let ctx =
            PackageBoxDecodeContext::new_inplace(recv.as_mut_ptr(), recv.len(), stack.keystore());
        match PackageBox::raw_decode_with_context(recv, ctx) {
            Ok((package_box, _)) => {
                if self.0.config.sn_only && !package_box.is_sn() {
                    return;
                }
                let local_interface = self.clone();
                if package_box.has_exchange() {
                    async_std::task::spawn(async move {
                        let exchange: &Exchange = package_box.packages()[0].as_ref();
                        if !exchange.verify(package_box.key()).await {
                            warn!("{} exchg verify failed, from {}.", local_interface, from);
                            return;
                        }
                        let _ = stack.on_udp_package_box(UdpPackageBox::new(
                            package_box,
                            local_interface,
                            from,
                        ));
                    });
                } else {
                    let _ = stack.on_udp_package_box(UdpPackageBox::new(
                        package_box,
                        local_interface,
                        from,
                    ));
                }
            }
            Err(err) => {
                // do nothing
                error!("{} decode failed, len={}, e={}", self, recv.len(), &err);
            }
        }
    }

    fn recv_loop(&self, weak_stack: WeakStack) {
        UDP_RECV_BUFFER.with(|thread_recv_buf| {
            let recv_buf = &mut thread_recv_buf.borrow_mut()[..];
            loop {
                let rr = self.0.socket.recv_from(recv_buf);
                if rr.is_ok() {
                    let stack = Stack::from(&weak_stack);
                    let (len, from) = rr.unwrap();
                    trace!("{} recv {} bytes from {}", self, len, from);
                    let recv = &mut recv_buf[..len];
                    // FIXME: ????????????????????????
                    self.on_recv(stack, recv, Endpoint::from((Protocol::Udp, from)));
                } else {
                    let err = rr.err().unwrap();
                    if let Some(10054i32) = err.raw_os_error() {
                        // In Windows, if host A use UDP socket and call sendto() to send something to host B,
                        // but B doesn't bind any port so that B doesn't receive the message,
                        // and then host A call recvfrom() to receive some message,
                        // recvfrom() will failed, and WSAGetLastError() will return 10054.
                        // It's a bug of Windows.
                        trace!("{} socket recv failed for {}, ingore this error", self, err);
                    } else {
                        info!("{} socket recv failed for {}, break recv loop", self, err);
                        break;
                    }
                }
            }
        });
    }

    pub fn send_box_to(
        &self,
        context: &mut PackageBoxEncodeContext,
        package_box: &PackageBox,
        to: &Endpoint,
    ) -> Result<usize, BuckyError> {
        if self.0.config.sn_only && !package_box.is_sn() {
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, "interface is only for sn"));
        } 
        BOX_CRYPTO_BUFFER.with(|thread_crypto_buf| {
            let crypto_buf = &mut thread_crypto_buf.borrow_mut()[..];
            let buf_len = crypto_buf.len();
            let next_ptr = package_box
                .raw_encode_with_context(crypto_buf, context, &None)
                .map_err(|e| {
                    error!("send_box_to encode failed, e:{}", &e);
                    e
                })?;
            let send_len = buf_len - next_ptr.len();
            self.send_buf_to(&crypto_buf[..send_len], to)
        })
    }

    pub fn send_box_mult(
        context: &mut PackageBoxEncodeContext,
        package_box: &PackageBox,
        iter: impl Iterator<Item = (Self, Endpoint)>,
        on_result: impl Fn(&Self, &Endpoint, Result<usize, BuckyError>) -> bool,
    ) -> Result<usize, BuckyError> {
        BOX_CRYPTO_BUFFER.with(|thread_crypto_buf| {
            let crypto_buf = &mut thread_crypto_buf.borrow_mut()[..];
            let buf_len = crypto_buf.len();
            let next_ptr = package_box
                .raw_encode_with_context(crypto_buf, context, &None)
                .map_err(|e| {
                    error!("send_box_mult encode failed, e:{}", &e);
                    e
                })?;
            let send_len = buf_len - next_ptr.len();
            let mut send_count = 0;
            for (from, to) in iter {
                if from.local().is_same_ip_version(&to) {
                    send_count += 1;
                    let is_continue =
                        on_result(&from, &to, from.send_buf_to(&crypto_buf[..send_len], &to));
                    if !is_continue {
                        break;
                    }
                }
            }
            Ok(send_count)
        })
    }

    pub fn send_raw_data_to(
        &self,
        key: &AesKey,
        data: &mut [u8],
        to: &Endpoint,
    ) -> Result<usize, BuckyError> {
        if self.0.config.sn_only {
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, "interface is only for sn"));
        } 
        let mix_hash = key.mix_hash(None);
        let _ = mix_hash.raw_encode(data, &None)?;
        data[0] |= 0x80;
        self.send_buf_to(data, to)
    }

    pub fn send_buf_to(&self, buf: &[u8], to: &Endpoint) -> Result<usize, BuckyError> {
        trace!("{} send {} bytes to {}", self, buf.len(), to);
        if self.0.config.sim_loss_rate > 0 {
            if rand::random::<u8>() < self.0.config.sim_loss_rate {
                trace!("{} sim loss", self);
                return Ok(buf.len());
            }
        }
        self.0
            .socket
            .send_to(buf, to.addr())
            .map_err(|e| BuckyError::from(e))
    }
}

pub struct PackageBoxEncodeContext {
    ignore_exchange: bool, 
    remote_const: Option<DeviceDesc>,
    fixed_values: merge_context::FixedValues,
    merged_values: Option<merge_context::ContextNames>,
}

impl PackageBoxEncodeContext {
    pub fn set_ignore_exchange(&mut self, b: bool) {
        self.ignore_exchange = b
    }
}

impl From<&DeviceDesc> for PackageBoxEncodeContext {
    fn from(remote_const: &DeviceDesc) -> Self {
        Self {
            ignore_exchange: false, 
            remote_const: Some(remote_const.clone()),
            fixed_values: merge_context::FixedValues::new(),
            merged_values: None,
        }
    }
}

// ??????SnCall::payload
impl From<(&DeviceDesc, &SnCall)> for PackageBoxEncodeContext {
    fn from(params: (&DeviceDesc, &SnCall)) -> Self {
        let fixed_values: merge_context::FixedValues = params.1.into();
        let merged_values = fixed_values.clone_merged();
        Self {
            ignore_exchange: false, 
            remote_const: Some(params.0.clone()),
            fixed_values,
            merged_values: Some(merged_values),
        }
    }
}

// impl From<(&DeviceDesc, Timestamp)> for PackageBoxEncodeContext {
//     fn from(params: (&DeviceDesc, Timestamp)) -> Self {
//         let mut fixed_values = merge_context::FixedValues::new();
//         fixed_values.insert("send_time", params.1);
//         Self {
//             ignore_exchange: false, 
//             remote_const: Some(params.0.clone()),
//             fixed_values,
//             merged_values: None,
//         }
//     }
// }

impl Default for PackageBoxEncodeContext {
    fn default() -> Self {
        Self {
            ignore_exchange: false, 
            remote_const: None,
            fixed_values: merge_context::FixedValues::new(),
            merged_values: None,
        }
    }
}

enum DecryptBuffer<'de> {
    Copy(&'de mut [u8]),
    Inplace(*mut u8, usize),
}

pub struct PackageBoxDecodeContext<'de> {
    decrypt_buf: DecryptBuffer<'de>,
    keystore: &'de keystore::Keystore,
}

impl<'de> PackageBoxDecodeContext<'de> {
    pub fn new_copy(decrypt_buf: &'de mut [u8], keystore: &'de keystore::Keystore) -> Self {
        Self {
            decrypt_buf: DecryptBuffer::Copy(decrypt_buf),
            keystore,
        }
    }

    pub fn new_inplace(ptr: *mut u8, len: usize, keystore: &'de keystore::Keystore) -> Self {
        Self {
            decrypt_buf: DecryptBuffer::Inplace(ptr, len),
            keystore,
        }
    }

    // ????????????aes ?????????buffer
    pub unsafe fn decrypt_buf(self, data: &[u8]) -> &'de mut [u8] {
        use DecryptBuffer::*;
        match self.decrypt_buf {
            Copy(decrypt_buf) => {
                decrypt_buf[..data.len()].copy_from_slice(data);
                decrypt_buf
            }
            Inplace(ptr, len) => {
                std::slice::from_raw_parts_mut(ptr.offset((len - data.len()) as isize), data.len())
            }
        }
    }
    // ??????local??????
    pub fn local_secret(&self) -> &PrivateKey {
        self.keystore.private_key()
    }

    pub fn local_public_key(&self) -> &PublicKey {
        self.keystore.public_key()
    }

    pub fn key_from_mixhash(&self, mix_hash: &KeyMixHash) -> Option<(DeviceId, AesKey)> {
        self.keystore
            .get_key_by_mix_hash(mix_hash, true, true)
            .map(|k| (k.peerid.clone(), k.aes_key.clone()))
    }
}

impl RawEncodeWithContext<PackageBoxEncodeContext> for PackageBox {
    fn raw_measure_with_context(
        &self,
        _: &mut PackageBoxEncodeContext,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        //TODO
        Ok(2048)
    }
    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        context: &mut PackageBoxEncodeContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut buf = buf;
        if self.has_exchange() && !context.ignore_exchange {
            let remote_const = match context.remote_const.as_ref() {
                Some(c) => c,
                None => {
                    log::error!("try encode exchange without public-key");
                    assert!(false);
                    return Err(BuckyError::new(
                        BuckyErrorCode::Failed,
                        "try encode exchange without public-key",
                    ));
                }
            };
            // ??????????????????const info??????aes key
            let key_len = self.key().raw_measure(purpose)?;
            let mut key_buf = vec![0; key_len];
            self.key().raw_encode(key_buf.as_mut_slice(), &None)?;
            let encrypt_len = remote_const
                .public_key()
                .encrypt(&key_buf.as_slice(), buf)?;
            buf = &mut buf[encrypt_len..];
        }

        // ?????? key???mixhash
        let mixhash = self.key().mix_hash(None);
        let buf = mixhash.raw_encode(buf, purpose)?;

        let mut encrypt_in_len = buf.len();
        let to_encrypt_buf = buf;

        // ???????????????
        let packages = if context.ignore_exchange {
            self.packages_no_exchange()
        } else {
            self.packages()
        };
        let (mut other_context, mut buf, packages) = match context.merged_values.as_ref() {
            Some(merged_values) => (
                merge_context::OtherEncode::new(merged_values.clone(), Some(&context.fixed_values)),
                &mut to_encrypt_buf[..],
                packages,
            ),
            None => {
                let mut first_context = merge_context::FirstEncode::from(&context.fixed_values); // merge_context::FirstEncode::new();
                let enc: &dyn RawEncodeWithContext<merge_context::FirstEncode> =
                    packages.get(0).unwrap().as_ref();
                let buf = enc.raw_encode_with_context(to_encrypt_buf, &mut first_context, purpose)?;
                (first_context.into(), buf, &packages[1..])
            }
        };

        for p in packages {
            let enc: &dyn RawEncodeWithContext<merge_context::OtherEncode> = p.as_ref();
            buf = enc.raw_encode_with_context(buf, &mut other_context, purpose)?;
        }
        encrypt_in_len -= buf.len();
        // ???aes ??????package?????????
        let len = self.key().inplace_encrypt(to_encrypt_buf, encrypt_in_len)?;
        Ok(&mut to_encrypt_buf[len..])
    }
}

impl<'de> RawDecodeWithContext<'de, PackageBoxDecodeContext<'de>> for PackageBox {
    fn raw_decode_with_context(
        buf: &'de [u8],
        context: PackageBoxDecodeContext<'de>,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        Self::raw_decode_with_context(buf, (context, None))
    }
}

impl<'de>
    RawDecodeWithContext<
        'de,
        (
            PackageBoxDecodeContext<'de>,
            Option<merge_context::OtherDecode>,
        ),
    > for PackageBox
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        c: (
            PackageBoxDecodeContext<'de>,
            Option<merge_context::OtherDecode>,
        ),
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let (context, mut merged_values) = c;
        let (mix_hash, hash_buf) = KeyMixHash::raw_decode(buf)?;
        let ((remote, aes_key, _mix_hash), buf) = {
            match context.key_from_mixhash(&mix_hash) {
                Some((remote, key)) => Ok(((Some(remote), key, mix_hash), hash_buf)),
                None => {
                    let encrypt_key_size = context.local_public_key().key_size();
                    if buf.len() < encrypt_key_size {
                        let msg = format!("not enough buffer for encrypt_key_size, except={}, got={}", encrypt_key_size, buf.len());
                        error!("{}", msg);

                        Err(BuckyError::new(
                            BuckyErrorCode::InvalidData,
                            msg,
                        ))
                    } else {
                        let mut key = AesKey::default();
                        let aeskey_len = context
                            .local_secret()
                            .decrypt(&buf[..encrypt_key_size], key.as_mut_slice())?;
                        if aeskey_len < key.raw_measure(&None).unwrap() {
                            Err(BuckyError::new(
                                BuckyErrorCode::InvalidData,
                                "invalid aeskey",
                            ))
                        } else {
                            let (mix_hash, buf) = KeyMixHash::raw_decode(&buf[encrypt_key_size..])?;
                            Ok(((None, key, mix_hash), buf))
                        }
                    }
                }
            }
        }?;
        // ?????????????????????context ??????buffer??????
        let decrypt_buf = unsafe { context.decrypt_buf(buf) };
        // ???key ????????????
        let decrypt_len = aes_key.inplace_decrypt(decrypt_buf, buf.len())?;
        let remain_buf = &buf[buf.len()..];
        let decrypt_buf = &decrypt_buf[..decrypt_len];

        let mut packages = vec![];

        //????????????package
        if decrypt_len != 0 {
            match merged_values.as_mut() {
                Some(merged) => {
                    let (package, buf) =
                        DynamicPackage::raw_decode_with_context(decrypt_buf, merged)?;
                    packages.push(package);
                    let mut buf_ptr = buf;
                    while buf_ptr.len() > 0 {
                        let (package, buf) =
                            DynamicPackage::raw_decode_with_context(buf_ptr, merged)?;
                        buf_ptr = buf;
                        packages.push(package);
                    }
                }
                None => {
                    let mut context = merge_context::FirstDecode::new();
                    let (package, buf) = DynamicPackage::raw_decode_with_context(
                        decrypt_buf[0..decrypt_len].as_ref(),
                        &mut context,
                    )?;
                    packages.push(package);
                    let mut context: merge_context::OtherDecode = context.into();
                    let mut buf_ptr = buf;
                    while buf_ptr.len() > 0 {
                        let (package, buf) =
                            DynamicPackage::raw_decode_with_context(buf_ptr, &mut context)?;
                        buf_ptr = buf;
                        packages.push(package);
                    }
                }
            }
        }

        match remote {
            Some(remote) => {
                let mut package_box = PackageBox::encrypt_box(remote, aes_key);
                package_box.append(packages);
                Ok((package_box, remain_buf))
            }
            None => {
                if packages.len() > 0 && packages[0].cmd_code().is_exchange() {
                    let exchange: &Exchange = packages[0].as_ref();
                    let mut package_box =
                        PackageBox::encrypt_box(exchange.from_device_id.clone(), aes_key);
                    package_box.append(packages);
                    Ok((package_box, remain_buf))
                } else {
                    Err(BuckyError::new(BuckyErrorCode::InvalidData, "unkown from"))
                }
            }
        }
    }
}
