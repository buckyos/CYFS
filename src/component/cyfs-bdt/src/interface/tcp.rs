use async_std::{
    future,
    io::prelude::{ReadExt, WriteExt},
    net::{Shutdown, TcpStream},
    sync::Arc,
    task,
};
use log::*;
use std::{
    convert::TryFrom, io::ErrorKind, net::TcpListener, sync::RwLock, thread, time::Duration,
};
//
// use socket2;
use super::{manager::UpdateOuterResult, udp};
use crate::{
    history::keystore,
    protocol::*,
    stack::{Stack, WeakStack},
};
use cyfs_base::endpoint;
use cyfs_base::*;

struct ListenerImpl {
    local: RwLock<Endpoint>,
    outer: RwLock<Option<Endpoint>>,
    socket: TcpListener,
    mapping_port: Option<u16>,
}
#[derive(Clone)]
pub struct Listener(Arc<ListenerImpl>);

pub trait OnTcpInterface {
    fn on_tcp_interface(
        &self,
        interface: AcceptInterface,
        first_box: PackageBox,
    ) -> Result<OnPackageResult, BuckyError>;
}

impl std::fmt::Display for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TcpListener {{local:{}}}", self.local())
    }
}

impl Listener {
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
                UpdateOuterResult::Update
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

    pub fn mapping_port(&self) -> Option<u16> {
        self.0.mapping_port
    }

    pub fn bind(local: &Endpoint, mapping_port: Option<u16>) -> Result<Self, BuckyError> {
        let socket = {
            if local.addr().is_ipv6() {
                #[cfg(windows)]
                {
                    let mut default_local = Endpoint::default_tcp(local);
                    default_local.mut_addr().set_port(local.addr().port());
                    TcpListener::bind(&default_local).map_err(|err| BuckyError::from(err))
                }
                #[cfg(not(windows))]
                {
                    use std::os::unix::io::FromRawFd;
                    unsafe {
                        let raw_sock = libc::socket(libc::AF_INET6, libc::SOCK_STREAM, 0);
                        let yes: libc::c_int = 1;
                        libc::setsockopt(
                            raw_sock,
                            libc::IPPROTO_IPV6,
                            libc::IPV6_V6ONLY,
                            &yes as *const libc::c_int as *const libc::c_void,
                            std::mem::size_of::<libc::c_int>().try_into().unwrap(),
                        );
                        let addr = libc::sockaddr_in6 {
                            #[cfg(any(target_os = "macos", target_os = "ios"))]
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
                            Ok(TcpListener::from_raw_fd(raw_sock))
                        }
                    }
                }
            } else if local.is_sys_default() {
                let mut default_local = Endpoint::default_tcp(local);
                default_local.mut_addr().set_port(local.addr().port());
                TcpListener::bind(&default_local).map_err(|err| BuckyError::from(err))
            } else {
                TcpListener::bind(local).map_err(|err| BuckyError::from(err))
            }
        }?;

        Ok(Self(Arc::new(ListenerImpl {
            local: RwLock::new(local.clone()),
            outer: RwLock::new(None),
            socket,
            mapping_port,
        })))
    }

    pub fn start(&self, stack: WeakStack) {
        let socket = self.0.socket.try_clone().unwrap();
        thread::spawn(move || loop {
            let stack = stack.clone();
            let key_store = Stack::from(&stack).keystore().clone();
            match socket.accept() {
                Ok((socket, _from_addr)) => {
                    task::spawn(async move {
                        let socket = TcpStream::from(socket);
                        match AcceptInterface::accept(
                            socket.clone(),
                            &key_store,
                            Stack::from(&stack).config().tunnel.tcp.accept_timeout,
                        )
                        .await
                        {
                            Ok((interface, first_box)) => {
                                let _ = Stack::from(&stack).on_tcp_interface(interface, first_box);
                            }
                            Err(e) => {
                                warn!("tcp-listener accept a stream, but the first package read failed. err: {}", e);
                                let _ = socket.shutdown(Shutdown::Both);
                            }
                        }
                    });
                }
                Err(e) => match e.kind() {
                    ErrorKind::Interrupted
                    | ErrorKind::WouldBlock
                    | ErrorKind::AlreadyExists
                    | ErrorKind::TimedOut => continue,
                    _ => {
                        warn!("tcp-listener accept fatal error({}). will stop.", e);
                        break;
                    }
                },
            }
        });
    }

    pub fn reset(&self, local: &Endpoint) -> Self {
        info!("{} reset with {}", self, local);
        *self.0.local.write().unwrap() = local.clone();
        *self.0.outer.write().unwrap() = None;
        self.clone()
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
}

type FirstBoxEncodeContext = udp::PackageBoxEncodeContext;
type FirstBoxDecodeContext<'de> = udp::PackageBoxDecodeContext<'de>;

pub(crate) struct OtherBoxEncodeContext {}

impl RawEncodeWithContext<OtherBoxEncodeContext> for PackageBox {
    fn raw_measure_with_context(
        &self,
        _: &mut OtherBoxEncodeContext,
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        unimplemented!()
    }
    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        _context: &mut OtherBoxEncodeContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let mut encrypt_in_len = buf.len();
        let to_encrypt_buf = buf;

        // 编码所有包
        let mut context = merge_context::FirstEncode::new();
        let packages = self.packages();
        let enc: &dyn RawEncodeWithContext<merge_context::FirstEncode> =
            packages.get(0).unwrap().as_ref();
        let mut buf = enc.raw_encode_with_context(to_encrypt_buf, &mut context, purpose)?;

        let mut context: merge_context::OtherEncode = context.into();
        for p in &packages[1..] {
            let enc: &dyn RawEncodeWithContext<merge_context::OtherEncode> = p.as_ref();
            buf = enc.raw_encode_with_context(buf, &mut context, purpose)?;
        }
        encrypt_in_len -= buf.len();
        // 用aes 加密package的部分
        let len = self.key().inplace_encrypt(to_encrypt_buf, encrypt_in_len)?;
        Ok(&mut to_encrypt_buf[len..])
    }
}

enum DecryptBuffer<'de> {
    Copy(&'de mut [u8]),
    Inplace(*mut u8, usize),
}

struct OtherBoxDecodeContext<'de> {
    decrypt_buf: DecryptBuffer<'de>,
    remote: &'de DeviceId,
    key: &'de AesKey,
}

impl<'de> OtherBoxDecodeContext<'de> {
    pub fn new_copy(decrypt_buf: &'de mut [u8], remote: &'de DeviceId, key: &'de AesKey) -> Self {
        Self {
            decrypt_buf: DecryptBuffer::Copy(decrypt_buf),
            remote,
            key,
        }
    }

    pub fn new_inplace(ptr: *mut u8, len: usize, remote: &'de DeviceId, key: &'de AesKey) -> Self {
        Self {
            decrypt_buf: DecryptBuffer::Inplace(ptr, len),
            remote,
            key,
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

    pub fn remote(&self) -> &DeviceId {
        self.remote
    }

    pub fn key(&self) -> &AesKey {
        self.key
    }
}

impl<'de> RawDecodeWithContext<'de, OtherBoxDecodeContext<'de>> for PackageBox {
    fn raw_decode_with_context(
        buf: &'de [u8],
        context: OtherBoxDecodeContext<'de>,
    ) -> BuckyResult<(Self, &'de [u8])> {
        let key = context.key().clone();
        let remote = context.remote().clone();

        let decrypt_buf = unsafe { context.decrypt_buf(buf) };
        // 用key 解密数据
        let decrypt_len = key.inplace_decrypt(decrypt_buf, buf.len())?;
        let remain_buf = &buf[buf.len()..];
        let decrypt_buf = &decrypt_buf[..decrypt_len];
        let mut packages = vec![];

        {
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

        let mut package_box = PackageBox::encrypt_box(remote, key);
        package_box.append(packages);
        Ok((package_box, remain_buf))
    }
}

struct PackageBoxEncodeContext<InnerContext>(InnerContext);
struct PackageBoxDecodeContext<InnerContext>(InnerContext);

impl RawEncodeWithContext<PackageBoxEncodeContext<FirstBoxEncodeContext>> for PackageBox {
    fn raw_measure_with_context(
        &self,
        _: &mut PackageBoxEncodeContext<FirstBoxEncodeContext>,
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        unimplemented!()
    }
    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        context: &mut PackageBoxEncodeContext<FirstBoxEncodeContext>,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf_len = buf.len();
        let box_header_len = u16::raw_bytes().unwrap();
        if buf_len < box_header_len {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "buffer not enough",
            ));
        }

        let box_len = {
            let buf_ptr =
                self.raw_encode_with_context(&mut buf[box_header_len..], &mut context.0, purpose)?;
            buf_len - buf_ptr.len() - box_header_len
        };
        let buf = (box_len as u16).raw_encode(buf, purpose)?;
        Ok(&mut buf[box_len..])
    }
}

impl RawEncodeWithContext<PackageBoxEncodeContext<OtherBoxEncodeContext>> for PackageBox {
    fn raw_measure_with_context(
        &self,
        _: &mut PackageBoxEncodeContext<OtherBoxEncodeContext>,
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        unimplemented!()
    }
    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        context: &mut PackageBoxEncodeContext<OtherBoxEncodeContext>,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf_len = buf.len();
        let box_header_len = u16::raw_bytes().unwrap();
        if buf_len < box_header_len {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "buffer not enough",
            ));
        }

        let box_len = {
            let buf_ptr =
                self.raw_encode_with_context(&mut buf[box_header_len..], &mut context.0, purpose)?;
            buf_len - buf_ptr.len() - box_header_len
        };
        let buf = (box_len as u16).raw_encode(buf, purpose)?;
        Ok(&mut buf[box_len..])
    }
}

impl<'de> RawDecodeWithContext<'de, PackageBoxDecodeContext<FirstBoxDecodeContext<'de>>>
    for PackageBox
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        context: PackageBoxDecodeContext<FirstBoxDecodeContext<'de>>,
    ) -> BuckyResult<(Self, &'de [u8])> {
        let (_box_len, buf) = u16::raw_decode(buf)?;
        PackageBox::raw_decode_with_context(buf, context.0)
    }
}

impl<'de> RawDecodeWithContext<'de, PackageBoxDecodeContext<OtherBoxDecodeContext<'de>>>
    for PackageBox
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        context: PackageBoxDecodeContext<OtherBoxDecodeContext<'de>>,
    ) -> BuckyResult<(Self, &'de [u8])> {
        let (_box_len, buf) = u16::raw_decode(buf)?;
        PackageBox::raw_decode_with_context(buf, context.0)
    }
}

#[derive(Eq, PartialEq)]
enum BoxType {
    Package,
    RawData,
}

async fn receive_box<'a>(
    socket: &TcpStream,
    recv_buf: &'a mut [u8],
) -> Result<(BoxType, &'a mut [u8]), BuckyError> {
    let mut socket = socket.clone();
    let header_len = u16::raw_bytes().unwrap();
    let box_header = &mut recv_buf[..header_len];
    socket.read_exact(box_header).await?;
    let mut box_len = u16::raw_decode(box_header).map(|(v, _)| v as usize)?;
    let box_type = if box_len > 32768 {
        box_len -= 32768;
        BoxType::RawData
    } else {
        BoxType::Package
    };
    if box_len + header_len > recv_buf.len() {
        return Err(BuckyError::new(
            BuckyErrorCode::OutOfLimit,
            "buffer not enough",
        ));
    }
    let box_buf = &mut recv_buf[header_len..(header_len + box_len)];
    socket.read_exact(box_buf).await?;
    Ok((box_type, box_buf))
}

struct AcceptInterfaceImpl {
    socket: TcpStream,
    remote_device_id: DeviceId,
    local: Endpoint,
    remote: Endpoint,
    key: AesKey,
}

#[derive(Clone)]
pub struct AcceptInterface(Arc<AcceptInterfaceImpl>);

impl std::fmt::Display for AcceptInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AcceptInterface{{local:{},remote:{},remote_device_id:{}}}",
            self.0.local, self.0.remote, self.0.remote_device_id
        )
    }
}

impl AcceptInterface {
    pub(crate) async fn accept(
        socket: TcpStream,
        keystore: &keystore::Keystore,
        timeout: Duration,
    ) -> Result<(Self, PackageBox), BuckyError> {
        let remote = socket.peer_addr().map_err(|e| BuckyError::from(e))?;
        let local = socket.local_addr().map_err(|e| BuckyError::from(e))?;
        let remote = Endpoint::from((endpoint::Protocol::Tcp, remote));
        let local = Endpoint::from((endpoint::Protocol::Tcp, local));

        let mut recv_buf = [0u8; udp::MTU];
        let (box_type, box_buf) =
            future::timeout(timeout, receive_box(&socket, &mut recv_buf)).await??;
        if box_type != BoxType::Package {
            let msg = format!("recv first box raw data from {}", remote);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }
        let first_box = {
            let context =
                FirstBoxDecodeContext::new_inplace(box_buf.as_mut_ptr(), box_buf.len(), keystore);
            PackageBox::raw_decode_with_context(box_buf, context)
                .map(|(package_box, _)| package_box)?
        };

        let exchg = match first_box.packages().get(0) {
            Some(first_pkg) => match first_pkg.cmd_code() {
                PackageCmdCode::Exchange => first_pkg.as_any().downcast_ref::<Exchange>(),
                _ => None,
            },
            None => return Err(BuckyError::new(BuckyErrorCode::InvalidData, "no package")),
        };
        if let Some(exchg) = exchg {
            if !exchg.verify(first_box.key()).await {
                warn!("tcp exchg verify failed.");
                return Err(BuckyError::new(
                    BuckyErrorCode::InvalidData,
                    "sign-verify failed",
                ));
            }
        }

        Ok((
            Self(Arc::new(AcceptInterfaceImpl {
                socket,
                key: first_box.key().clone(),
                remote_device_id: first_box.remote().clone(),
                local,
                remote,
            })),
            first_box,
        ))
    }

    pub fn socket(&self) -> &TcpStream {
        &self.0.socket
    }

    pub fn key(&self) -> &AesKey {
        &self.0.key
    }

    pub fn remote_device_id(&self) -> &DeviceId {
        &self.0.remote_device_id
    }

    pub fn remote(&self) -> &Endpoint {
        &self.0.remote
    }

    pub fn local(&self) -> &Endpoint {
        &self.0.local
    }

    pub async fn confirm_accept(&self, packages: Vec<DynamicPackage>) -> Result<(), BuckyError> {
        let mut send_buffer = [0u8; udp::MTU];
        let mut context = PackageBoxEncodeContext(OtherBoxEncodeContext {});
        let mut package_box =
            PackageBox::encrypt_box(self.remote_device_id().clone(), self.0.key.clone());
        package_box.append(packages);
        let mut socket = self.socket().clone();
        socket
            .write_all(package_box.raw_tail_encode_with_context(
                &mut send_buffer,
                &mut context,
                &None,
            )?)
            .await?;
        Ok(())
    }
}

impl Into<PackageInterface> for AcceptInterface {
    fn into(self) -> PackageInterface {
        PackageInterface(Arc::new(PackageInterfaceImpl {
            local: self.0.local,
            remote: self.0.remote,
            socket: self.0.socket.clone(),
            key: self.0.key.clone(),
            remote_device_id: self.0.remote_device_id.clone(),
        }))
    }
}

struct InterfaceImpl {
    socket: TcpStream,
    remote_device_id: DeviceId,
    local: Endpoint,
    remote: Endpoint,
    remote_device_desc: DeviceDesc,
    key: AesKey,
}

impl std::fmt::Display for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TcpInterface{{local:{},remote:{}}}",
            self.0.local, self.0.remote
        )
    }
}

#[derive(Clone)]
pub struct Interface(Arc<InterfaceImpl>);

impl Interface {
    pub async fn connect(
        /*from_ip: IpAddr,*/
        remote_ep: Endpoint,
        remote_device_id: DeviceId,
        remote_device_desc: DeviceDesc,
        key: AesKey,
        timeout: Duration,
    ) -> Result<Interface, BuckyError> {
        // let socket = socket2::Socket::new(socket2::Domain::ipv4(), socket2::Type::stream(), None).unwrap();
        // socket.bind(&socket2::SockAddr::from(SocketAddr::new(from_ip, 0))).map_err(|err| {
        //     debug!("bind tcp socket failed for {}", err);
        //     BuckyError::new(BuckyErrorCode::Failed, format!("bind tcp socket failed for {}", err))
        // })?;
        // socket.connect_timeout(&socket2::SockAddr::from(*remote_ep.addr()), timeout).map_err(|err| {
        //     debug!("tcp socket from {} to {} connect failed for {}", from_ip, remote_ep, err);
        //     BuckyError::new(BuckyErrorCode::Failed, format!("tcp socket from {} to {} connect failed for {}", from_ip, remote_ep, err))
        // })?;
        // let socket = TcpStream::from(socket.into_tcp_stream());
        let socket = async_std::io::timeout(timeout, TcpStream::connect(remote_ep.addr()))
            .await
            .map_err(|err| {
                debug!(
                    "tcp socket to {} connect failed for {}",
                    /*from_ip, */ remote_ep, err
                );
                err
            })?;
        let local = socket.local_addr().map_err(|e| BuckyError::from(e))?;
        let local = Endpoint::from((endpoint::Protocol::Tcp, local));
        let interface = Interface(Arc::new(InterfaceImpl {
            socket,
            local,
            remote: remote_ep,
            remote_device_id,
            remote_device_desc,
            key,
        }));
        debug!("{} connected", interface);
        Ok(interface)
    }

    pub async fn confirm_connect(
        &self,
        stack: &Stack,
        mut packages: Vec<DynamicPackage>,
        timeout: Duration,
    ) -> Result<PackageBox, BuckyError> {
        debug!("{} confirm_connect", self);
        let key_stub = stack
            .keystore()
            .get_key_by_mix_hash(&self.key().mix_hash(None), false, false)
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::CryptoError, "key not exists"))?;
        let mut buffer = [0u8; udp::MTU];
        let mut package_box =
            PackageBox::encrypt_box(self.0.remote_device_id.clone(), self.0.key.clone());
        if !key_stub.is_confirmed {
            if let PackageCmdCode::Exchange = packages[0].cmd_code() {
                let exchg: &mut Exchange = packages[0].as_mut();
                exchg.sign(&self.0.key, stack.keystore().signer()).await?;
            } else {
                let mut exchg = Exchange::try_from(&packages[0])?;
                exchg.sign(&self.0.key, stack.keystore().signer()).await?;
                package_box.push(exchg);
            }
        }
        package_box.append(packages);

        let send_buf = {
            let mut context =
                PackageBoxEncodeContext(FirstBoxEncodeContext::from(&self.0.remote_device_desc));
            package_box.raw_tail_encode_with_context(&mut buffer, &mut context, &None)?
        };

        let mut socket = self.socket().clone();
        socket.write_all(send_buf).await.map_err(|err| {
            debug!("{} send first box failed for {}", self, err);
            err
        })?;
        debug!("{} first box sent {} bytes", self, send_buf.len());

        let (box_type, box_buf) =
            future::timeout(timeout, receive_box(&self.0.socket, &mut buffer))
                .await
                .map_err(|err| {
                    debug!("{} recv first box failed for {}", self, err);
                    err
                })?
                .map_err(|err| {
                    debug!("{} recv first box failed for {}", self, err);
                    err
                })?;
        if box_type != BoxType::Package {
            let msg = format!("{} recv first box raw data", self);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }
        let context = OtherBoxDecodeContext::new_inplace(
            box_buf.as_mut_ptr(),
            box_buf.len(),
            &self.0.remote_device_id,
            self.key(),
        );
        PackageBox::raw_decode_with_context(box_buf, context).map(|(package_box, _)| package_box)
    }

    pub fn socket(&self) -> &TcpStream {
        &self.0.socket
    }

    pub fn key(&self) -> &AesKey {
        &self.0.key
    }

    pub fn remote_endpoint(&self) -> &Endpoint {
        &self.0.remote
    }
}

impl Into<PackageInterface> for Interface {
    fn into(self) -> PackageInterface {
        PackageInterface(Arc::new(PackageInterfaceImpl {
            local: self.0.local,
            remote: self.0.remote,
            socket: self.0.socket.clone(),
            key: self.0.key.clone(),
            remote_device_id: self.0.remote_device_id.clone(),
        }))
    }
}

struct PackageInterfaceImpl {
    local: Endpoint,
    remote: Endpoint,
    socket: TcpStream,
    key: AesKey,
    remote_device_id: DeviceId,
}
#[derive(Clone)]
pub struct PackageInterface(Arc<PackageInterfaceImpl>);

impl std::fmt::Display for PackageInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PackageInterface{{local:{},remote:{}}}",
            self.0.local, self.0.remote
        )
    }
}

pub enum RecvBox<'a> {
    Package(PackageBox),
    RawData(&'a [u8]),
}

impl PackageInterface {
    pub fn ptr_eq(&self, other: &Self) -> bool {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawSocket;
            self.0.socket.as_raw_socket() == other.0.socket.as_raw_socket()
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::io::AsRawFd;
            self.0.socket.as_raw_fd() == other.0.socket.as_raw_fd()
        }
    }

    pub fn raw_header_data_len() -> usize {
        u16::raw_bytes().unwrap()
    }

    pub async fn receive_package<'a>(
        &'a self,
        recv_buf: &'a mut [u8],
    ) -> Result<RecvBox<'a>, BuckyError> {
        let (box_type, box_buf) = receive_box(&self.0.socket, recv_buf).await?;

        match box_type {
            BoxType::Package => {
                let context = OtherBoxDecodeContext::new_inplace(
                    box_buf.as_mut_ptr(),
                    box_buf.len(),
                    &self.0.remote_device_id,
                    &self.0.key,
                );
                let package = PackageBox::raw_decode_with_context(box_buf, context)
                    .map(|(package_box, _)| package_box)?;
                Ok(RecvBox::Package(package))
            }
            BoxType::RawData => Ok(RecvBox::RawData(box_buf)),
        }
    }

    pub async fn send_raw_buffer(&self, buffer: &mut [u8]) -> BuckyResult<()> {
        let data_len = buffer.len() - Self::raw_header_data_len() as usize;
        let _ = ((data_len + 32768) as u16).raw_encode(buffer, &None)?;
        let mut socket = self.0.socket.clone();
        socket.write_all(buffer).await?;
        Ok(())
    }

    pub async fn send_raw_data(&self, data: Vec<u8>) -> BuckyResult<()> {
        let mut data = data;
        self.send_raw_buffer(data.as_mut_slice()).await
    }

    pub async fn send_package<'a>(
        &self,
        send_buf: &'a mut [u8],
        package: DynamicPackage,
    ) -> Result<(), BuckyError> {
        let mut socket = self.0.socket.clone();
        let package_box =
            PackageBox::from_package(self.0.remote_device_id.clone(), self.0.key.clone(), package);
        let mut context = PackageBoxEncodeContext(OtherBoxEncodeContext {});
        socket
            .write_all(package_box.raw_tail_encode_with_context(send_buf, &mut context, &None)?)
            .await?;
        Ok(())
    }

    pub fn local(&self) -> BuckyResult<SocketAddr> {
        self.0.socket.local_addr().map_err(|e| BuckyError::from(e))
    }

    pub fn remote(&self) -> BuckyResult<SocketAddr> {
        self.0.socket.peer_addr().map_err(|e| BuckyError::from(e))
    }

    pub fn close(&self) {
        let _ = self.0.socket.shutdown(Shutdown::Both);
    }
}
