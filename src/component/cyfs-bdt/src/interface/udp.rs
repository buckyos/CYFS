use crate::{
    types::*, 
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
pub const MTU_LARGE: usize = 1024*30;

thread_local! {
    static UDP_RECV_BUFFER: RefCell<[u8; MTU_LARGE]> = RefCell::new([0u8; MTU_LARGE]);
    static BOX_CRYPTO_BUFFER: RefCell<[u8; MTU_LARGE]> = RefCell::new([0u8; MTU_LARGE]);
}

struct InterfaceImpl {
    config: Config, 
    socket: UdpSocket, 
    mapping_port: Option<u16>,
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

impl std::fmt::Debug for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UdpInterface {{local:{}}}", self.local())
    }
}



impl Interface {
    pub fn bind(local: Endpoint, out: Option<Endpoint>, mapping_port: Option<u16>, config: Config) -> Result<Self, BuckyError> {
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
                    let mut default_local = Endpoint::default_udp(&local);
                    default_local.mut_addr().set_port(local.addr().port());
                    match bind_socket(&default_local, config.recv_buffer) {
                        Ok(socket) => {
                            // 避免udp被对端reset
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
                        let mut default_local = Endpoint::default_udp(&local);
                        default_local.mut_addr().set_port(local.addr().port());
        
                        default_local
                    } else {
                        local
                    }
                };

                bind_socket(&bind_addr, config.recv_buffer)
            }
        }?;

        Ok(Self(Arc::new(InterfaceImpl {
            config, 
            mapping_port, 
            local: RwLock::new(local),
            socket,
            outer: RwLock::new(out),
        })))
    }


    pub fn mapping_port(&self) -> Option<u16> {
        self.0.mapping_port
    }

    pub fn reset(&self, local: &Endpoint) -> Self {
        info!("{} reset with {}", self, local);
        let new =  self.clone();
        *new.0.local.write().unwrap() = local.clone();
        *new.0.outer.write().unwrap() = None;
        new
    }

    pub fn start(&self, stack: WeakStack) {
        let ci = self.clone();
        thread::spawn(move || {
            info!("{} start on thread {:?}", ci, thread::current().id());
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
                            stack.on_udp_raw_data(raw_data, (self.clone(), found_key.peerid, found_key.key, from));

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
                        if !exchange.verify(stack.local_device_id()).await {
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
                error!("{} decode failed, from={}, len={}, e={}", self, from, recv.len(), &err);
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
                    // FIXME: 分发到工作线程去
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
                    error!("send_box_to encode failed, package_box: {:?}, e:{}", package_box, &e);
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
                    error!("send_box_mult encode failed, package_box: {:?}, e:{}", package_box, &e);
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
        key: &MixAesKey,
        data: &mut [u8],
        to: &Endpoint,
    ) -> Result<usize, BuckyError> {
        if self.0.config.sn_only {
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, "interface is only for sn"));
        }
        let mix_hash = key.mix_hash();
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
    plaintext: bool,
    ignore_exchange: bool, 
    fixed_values: merge_context::FixedValues,
    merged_values: Option<merge_context::ContextNames>,
}

impl PackageBoxEncodeContext {
    pub fn plaintext(&self) -> bool {
        self.plaintext
    }

    pub fn set_plaintext(&mut self, b: bool) {
        self.plaintext = b
    }

    pub fn set_ignore_exchange(&mut self, b: bool) {
        self.ignore_exchange = b
    }
}

// 编码SnCall::payload
impl From<&SnCall> for PackageBoxEncodeContext {
    fn from(sn_call: &SnCall) -> Self {
        let fixed_values: merge_context::FixedValues = sn_call.into();
        let merged_values = fixed_values.clone_merged();
        Self {
            plaintext: false,
            ignore_exchange: false, 
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
            plaintext: false,
            ignore_exchange: false, 
            fixed_values: merge_context::FixedValues::new(),
            merged_values: None,
        }
    }
}

enum DecryptBuffer<'de> {
    Copy(&'de mut [u8]),
    Inplace(*mut u8, usize),
}

pub trait PackageBoxVersionGetter {
    fn version_of(&self, remote: &DeviceId) -> u8; 
}

pub struct PackageBoxDecodeContext<'de> {
    decrypt_buf: DecryptBuffer<'de>,
    keystore: &'de keystore::Keystore, 
}

impl<'de> PackageBoxDecodeContext<'de> {
    pub fn new_copy(
        decrypt_buf: &'de mut [u8], 
        keystore: &'de keystore::Keystore, 
    ) -> Self {
        Self {
            decrypt_buf: DecryptBuffer::Copy(decrypt_buf),
            keystore, 
        }
    }

    pub fn new_inplace(
        ptr: *mut u8, 
        len: usize, 
        keystore: &'de keystore::Keystore, 
    ) -> Self {
        Self {
            decrypt_buf: DecryptBuffer::Inplace(ptr, len),
            keystore, 
        }
    }

    // 返回用于aes 解码的buffer
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
    // 拿到local私钥
    pub fn local_secret(&self) -> &PrivateKey {
        self.keystore.private_key()
    }

    pub fn local_public_key(&self) -> &PublicKey {
        self.keystore.public_key()
    }

    pub fn key_from_mixhash(&self, mix_hash: &KeyMixHash) -> Option<(DeviceId, MixAesKey)> {
        self.keystore
            .get_key_by_mix_hash(mix_hash, true, true)
            .map(|k| (k.peerid, k.key))
    }

    pub fn version_of(&self, _remote: &DeviceId) -> u8 {
        0
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
            let exchange: &Exchange = self.packages()[0].as_ref();
            if buf.len() < exchange.key_encrypted.len() {
                log::error!("try encode exchange without public-key");
                    assert!(false);
                    return Err(BuckyError::new(
                        BuckyErrorCode::Failed,
                        "try encode exchange without public-key",
                    ));
            }
            // 首先用对端的const info加密aes key
            buf[..exchange.key_encrypted.len()].copy_from_slice(&exchange.key_encrypted[..]);
            buf = &mut buf[exchange.key_encrypted.len()..];
        }

        // 写入 key的mixhash
        let mixhash = self.key().mix_hash();
        let _ = mixhash.raw_encode(buf, purpose)?;
        if context.plaintext {
            buf[0] |= 0x80;
        }
        let buf = &mut buf[8..];

        let mut encrypt_in_len = buf.len();
        let to_encrypt_buf = buf;

        // 编码所有包
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
        //let buf_len = buf.len();
        encrypt_in_len -= buf.len();
        // 用aes 加密package的部分
        let len = if context.plaintext {
            encrypt_in_len
        } else {
            self.key().enc_key.inplace_encrypt(to_encrypt_buf, encrypt_in_len)?
        };

        //info!("package_box udp encode: encrypt_in_len={} len={} buf_len={} plaintext={}", 
        //    encrypt_in_len, len, buf_len, context.plaintext);

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
        let (context, merged_values) = c;
        let (mix_hash, hash_buf) = KeyMixHash::raw_decode(buf)?;

        enum KeyStub {
            Exist(DeviceId), 
            Exchange(Vec<u8>)
        }

        struct KeyInfo {
            enc_key: AesKey, 
            mix_hash: KeyMixHash, 
            stub: KeyStub
        }


        let mut mix_key = None;
        let (key_info, buf) = {
            match context.key_from_mixhash(&mix_hash) {
                Some((remote, key)) => {
                        mix_key = Some(key.mix_key);

                    (KeyInfo {
                        stub: KeyStub::Exist(remote), 
                        enc_key: key.enc_key, 
                        mix_hash
                    }, hash_buf)
                }, 
                None => {
                    let mut enc_key = AesKey::default();
                    let (remain, _) = context.local_secret().decrypt_aeskey(buf, enc_key.as_mut_slice()).map_err(|e|{
                        error!("decrypt aeskey err={}. (maybe: 1. local/remote device time is not correct 2. the packet is broken 3. the packet not contains Exchange info etc.. )", e);
                        e
                    })?;
                    let encrypted = Vec::from(&buf[..buf.len() - remain.len()]);
                    let (mix_hash, remain) = KeyMixHash::raw_decode(remain)?;
                    (KeyInfo {
                        stub: KeyStub::Exchange(encrypted), 
                        enc_key, 
                        mix_hash, 
                    }, remain)
                }
            }
        };

        let mut version = if let KeyStub::Exist(remote) = &key_info.stub {
            context.version_of(remote)
        } else {
            0
        };
        // 把原数据拷贝到context 给的buffer上去
        let decrypt_buf = unsafe { context.decrypt_buf(buf) };
        // 用key 解密数据
        let decrypt_len =  key_info.enc_key.inplace_decrypt(decrypt_buf, buf.len())?;
        let remain_buf = &buf[buf.len()..];
        let decrypt_buf = &decrypt_buf[..decrypt_len];

        let mut packages = vec![];

        //解码所有package
        if decrypt_len != 0 {
            match merged_values {
                Some(mut merged) => {
                    let (package, buf) =
                        DynamicPackage::raw_decode_with_context(decrypt_buf, (&mut merged, &mut version))?;
                    packages.push(package);
                    let mut buf_ptr = buf;
                    while buf_ptr.len() > 0 {
                        match DynamicPackage::raw_decode_with_context(buf_ptr, (&mut merged, &mut version)) {
                            Ok((package, buf)) => {
                                buf_ptr = buf;
                                packages.push(package);
                            }, 
                            Err(err) => {
                                if err.code() == BuckyErrorCode::NotSupport {
                                    break;
                                } else {
                                    Err(err)?;
                                }
                            }
                        };
                    }
                }
                None => {
                    let mut context = merge_context::FirstDecode::new();
                    let (package, buf) = DynamicPackage::raw_decode_with_context(
                        decrypt_buf[0..decrypt_len].as_ref(),
                        (&mut context, &mut version)
                    )?;
                    packages.push(package);
                    let mut context: merge_context::OtherDecode = context.into();
                    let mut buf_ptr = buf;
                    while buf_ptr.len() > 0 {
                        match DynamicPackage::raw_decode_with_context(buf_ptr, (&mut context, &mut version)) {
                            Ok((package, buf)) => {
                                buf_ptr = buf;
                                packages.push(package);
                            }, 
                            Err(err) => {
                                if err.code() == BuckyErrorCode::NotSupport {
                                    break;
                                } else {
                                    Err(err)?;
                                }
                            }
                        };
                    }
                }
            }
        }

        if mix_key.is_none() {
            if packages.len() > 0 && packages[0].cmd_code().is_exchange() {
                let exchange: &Exchange = packages[0].as_ref();
                mix_key = Some(exchange.mix_key.clone());
            } else {
                return Err(BuckyError::new(BuckyErrorCode::ErrorState, "unkown mix_key"));
            }
        }

        let key = MixAesKey {
            enc_key: key_info.enc_key, 
            mix_key: mix_key.unwrap()
        };
        match key_info.stub {
            KeyStub::Exist(remote) => {
                let mut package_box = PackageBox::encrypt_box(remote,key );
                package_box.append(packages);
                Ok((package_box, remain_buf))
            }
            KeyStub::Exchange(encrypted) => {
                if packages.len() > 0 && packages[0].cmd_code().is_exchange() {
                    let exchange: &mut Exchange = packages[0].as_mut();
                    exchange.key_encrypted = encrypted;

                    let mut package_box =
                        PackageBox::encrypt_box(exchange.from_device_desc.desc().device_id(), key);
                    package_box.append(packages);
                    Ok((package_box, remain_buf))
                } else {
                    Err(BuckyError::new(BuckyErrorCode::InvalidData, "unkown from"))
                }
            }
        }
    }
}
