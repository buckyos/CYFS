// use log;
use std::{
    net::{UdpSocket, SocketAddr}, 
    cell::RefCell,  
    thread
};
use async_std::{
    sync::Arc
};
use cyfs_base::*;
use crate::{  
    protocol::{*, v0::*}, 
    interface::udp::*
};
use super::service::{Service, WeakService};

struct CommandTunnelImpl {
    service: WeakService, 
    socket: UdpSocket, 
}

#[derive(Clone)]
pub(super) struct CommandTunnel(Arc<CommandTunnelImpl>);

impl std::fmt::Display for CommandTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CommandTunnel:{{endpoint:{}}}", self.0.socket.local_addr().unwrap())
    }
}

thread_local! {
    static UDP_RECV_BUFFER: RefCell<[u8; MTU]> = RefCell::new([0u8; MTU]);
    static BOX_CRYPTO_BUFFER: RefCell<[u8; MTU]> = RefCell::new([0u8; MTU]);
}



impl CommandTunnel {
    pub(super) fn open(service: WeakService, local: Endpoint) -> BuckyResult<Self> {
        info!("command tunnel will listen on {:?}", local);
        let socket = UdpSocket::bind(local)
            .map_err(|e| {
                error!("command tunnel will listen on {:?} failed for {}", local, e);
                e
            })?;
        let tunnel = Self(Arc::new(CommandTunnelImpl {
            service, 
            socket, 
        }));
        let thread_count = 4;
        for _ in 0..thread_count {
            let tunnel = tunnel.clone();
            thread::spawn(move || {
                tunnel.listen();
            });
            
        }
        Ok(tunnel)
    }

    fn listen(&self) {
        UDP_RECV_BUFFER.with(|thread_recv_buf| {
            let recv_buf = &mut thread_recv_buf.borrow_mut()[..];
            loop {
                let rr = self.0.socket.recv_from(recv_buf);
                if rr.is_ok() {
                    let (len, from) = rr.unwrap();
                    let recv = &mut recv_buf[..len];
                    // FIXME: 分发到工作线程去
                    self.on_recv(recv, from);
                } else {
                    let err = rr.err().unwrap();
                    if let Some(10054i32) = err.raw_os_error() {
                        // In Windows, if host A use UDP socket and call sendto() to send something to host B,
                        // but B doesn't bind any port so that B doesn't receive the message,
                        // and then host A call recvfrom() to receive some message,
                        // recvfrom() will failed, and WSAGetLastError() will return 10054.
                        // It's a bug of Windows.
                    } else {
                        error!("{} socket recv failed for {}, break recv loop", self, err);
                        break;
                    }
                }
            }
        });
    }

    fn on_package_box(&self, package_box: PackageBox, from: SocketAddr) -> Result<(), BuckyError> {
        let packages = package_box.packages_no_exchange();
        if packages.len() != 1 {
            let e = BuckyError::new(BuckyErrorCode::InvalidInput, "package box contains multi packages");
            error!("{} ignore package box for {}", self, e);
            return Err(e);
        }
        let package = &packages[0];
        if package.cmd_code() != PackageCmdCode::SynProxy {
            let e = BuckyError::new(BuckyErrorCode::InvalidInput, "package box contains invalid package");
            error!("{} ignore package box for {}", self, e);
            return Err(e);
        }

        let syn_proxy: &SynProxy = package.as_ref();
        let service = Service::from(&self.0.service);
        let _ = service.on_package(syn_proxy, (&package_box, &from))?;
        Ok(())
    }

    
    fn on_recv(&self, recv: &mut [u8], from: SocketAddr) {
        let service = Service::from(&self.0.service);
        let ctx =
            PackageBoxDecodeContext::new_inplace(recv.as_mut_ptr(), recv.len(), service.keystore());
        match PackageBox::raw_decode_with_context(recv, ctx) {
            Ok((package_box, _)) => {
                let tunnel = self.clone();
                if package_box.has_exchange() {
                    async_std::task::spawn(async move {
                        let exchange: &Exchange = package_box.packages()[0].as_ref();
                        service.keystore().add_key(
                            package_box.enc_key(),
                            package_box.remote(),
                            &exchange.mix_key,
                        );
                        let _ = tunnel.on_package_box(package_box, from);
                    });
                } else {
                    let _ = self.on_package_box(package_box, from);
                }
            }, 
            Err(err) => {
                // do nothing
                error!("{} decode failed, from={}, len={}, e={}", self, from, recv.len(), &err);
            }
        }
    }


    pub(super) fn ack_proxy(
        &self, 
        proxy_endpoint: BuckyResult<SocketAddr>,  
        syn_proxy: &SynProxy, 
        to: &SocketAddr, 
        enc_key: &AesKey,
        mix_key: &AesKey) -> BuckyResult<()> {
        let (proxy_endpoint, err) = match proxy_endpoint {
            Ok(proxy_endpoint) => (Some(Endpoint::from((Protocol::Udp, proxy_endpoint))), None), 
            Err(err) => (None, Some(err.code()))
        };

        let ack_proxy = AckProxy {
            seq: syn_proxy.seq, 
            to_peer_id: syn_proxy.to_peer_id.clone(), 
            proxy_endpoint, 
            err
        };
        let mut package_box = PackageBox::encrypt_box(
            syn_proxy.from_peer_info.desc().device_id(), 
            enc_key.clone(), mix_key.clone());
        package_box.append(vec![DynamicPackage::from(ack_proxy)]);
        
        let mut context = PackageBoxEncodeContext::default();
        let _ = BOX_CRYPTO_BUFFER.with(|thread_crypto_buf| {
            let crypto_buf = &mut thread_crypto_buf.borrow_mut()[..];
            let buf_len = crypto_buf.len();
            let next_ptr = package_box
                .raw_encode_with_context(crypto_buf, &mut context, &None)
                .map_err(|e| {
                    error!("send_box_to encode failed, e:{}", &e);
                    e
                })?;
            let send_len = buf_len - next_ptr.len();
            self.0.socket.send_to(&crypto_buf[..send_len], to).map_err(|e| BuckyError::from(e))
        })?;

        Ok(())
    }
}

