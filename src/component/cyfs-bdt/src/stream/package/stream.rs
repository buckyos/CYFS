use log::*;
use std::{
    time::{Duration, Instant}, 
    task::{Context, Poll}, 
    sync::Mutex,
    collections::LinkedList,
};
use async_std::{
    sync::Arc,
    task, 
    future,
};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface,
    tunnel::{udp::Tunnel as UdpTunnel, tunnel::Tunnel, TunnelContainer}, 
    cc
};
use super::super::{
    container::StreamContainer, 
    stream_provider::{Shutdown, StreamProvider}};
use super::{
    write::WriteProvider,  
    read::ReadProvider,
};

#[derive(Clone)]
pub struct Config {
    pub connect_resend_interval: Duration, 
    pub atomic_interval: Duration, 
    pub break_overtime: Duration,  
    pub msl: Duration, 
    pub cc: cc::Config
}

struct PacePackage {
    send_time: Instant,
    package: DynamicPackage,
}

struct PackageStreamImpl {
    config: super::super::container::Config, 
    owner_disp: String, 
    tunnel: UdpTunnel, 
    local_id: IncreaseId, 
    remote_id: IncreaseId, 
    write_provider: WriteProvider, 
    read_provider: ReadProvider, 
    pacer: Mutex<cc::pacing::Pacer>, 
    package_queue: Arc<Mutex<LinkedList<PacePackage>>>, 
}

#[derive(Clone)]
pub struct PackageStream(Arc<PackageStreamImpl>);

impl std::fmt::Display for PackageStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PackageStream{{stream:{}, remote_id:{}}}", self.0.owner_disp, self.0.remote_id)
    }
}

impl PackageStream {
    pub fn mss() -> usize {
        interface::udp::MTU - 180
    }
    
    pub fn new(
        owner: &StreamContainer,
        tunnel: &TunnelContainer,
        local_id: IncreaseId, 
        remote_id: IncreaseId,
    ) -> BuckyResult<Self> {
        let owner_disp = format!("{}", owner);
        let config = tunnel.stack().config().stream.stream.clone();
	let pacer_enable = false;

        let write_provider = WriteProvider::new(&config);
        let read_provider = ReadProvider::new(&config);
        let stream = Self(Arc::new(PackageStreamImpl {
            owner_disp, 
            config, 
            tunnel: tunnel.default_udp_tunnel()?, 
            local_id, 
            remote_id, 
            write_provider,
            read_provider,
            pacer: Mutex::new(cc::pacing::Pacer::new(pacer_enable, PackageStream::mss() * 10, PackageStream::mss())),
            package_queue: Arc::new(Mutex::new(Default::default())),
        }));

        Ok(stream)
    }

    pub fn config(&self) -> &super::super::container::Config {
        &self.0.config
    }

    pub fn write_provider(&self) -> &WriteProvider {
        &self.0.write_provider
    }

    pub fn read_provider(&self) -> &ReadProvider {
        &self.0.read_provider
    }

    fn package_delay(&self, package: DynamicPackage, send_time: Instant) {
        let mut package_queue = self.0.package_queue.lock().unwrap();
        package_queue.push_back(PacePackage {
            send_time,
            package,
        });
    }

    fn drain_delay(&self) {
        let now = Instant::now();
        let mut package_queue = self.0.package_queue.lock().unwrap();
        let mut n = 0;
        for (_, package) in package_queue.iter().enumerate() {
            if package.send_time > now {
                break ;
            }
            n += 1;
        }

        while n > 0 {
            if let Some(package) = package_queue.pop_front() {
                match self.0.tunnel.send_package(package.package) {
                    Ok(sent_len) => {
                        trace!("package_delay send_package {}", sent_len);
                    },
                    Err(err) => {
                        error!("stream send_package err={}", err);
                    }
                }
            }
            n -= 1;
        }
    }

    pub fn send_packages(&self, packages: Vec<DynamicPackage>) -> Result<(), BuckyError> {
        if packages.len() == 0 {
            return Ok(());
        }
        trace!("{} send {} session data packages", self, packages.len());
        let mut ack = None;
        let mut packages = packages;
        for p in &mut packages {
            let session_data: &mut SessionData = p.as_mut();
            session_data.session_id = self.0.remote_id;
            if session_data.is_flags_contain(SESSIONDATA_FLAG_ACK) {
                if ack.is_none() {
                    ack = Some(self.read_provider().touch_ack(self));
                    trace!("{} touch ack {} fin {}", self, ack.as_ref().unwrap().0, ack.as_ref().unwrap().1);
                }
                session_data.ack_stream_pos = ack.as_ref().unwrap().0;
                if ack.as_ref().unwrap().1 {
                    session_data.flags_add(SESSIONDATA_FLAG_FINACK);
                }
            }
            // trace!("{} will send session data package {}", self, session_data);
        }
        
        let mut sent_bytes = 0;
        let mut last_packet_number = 0;
        {
            let mut pacer = self.0.pacer.lock().unwrap();
            let now = Instant::now();
            for package in packages {
                let session_data: & SessionData = package.as_ref();
                if !session_data.is_ctrl_package() || !session_data.is_flags_contain(SESSIONDATA_FLAG_ACK) {
                    let package_size = session_data.data_size();
                    if let Some(next_time) = pacer.send(package_size, now) {
                        sent_bytes += package_size;

                        self.package_delay(package, next_time);
                        continue;
                    }
                    last_packet_number = session_data.send_time;
                }

                match self.0.tunnel.send_package(package) {
                    Ok(sent_len) => {
                        sent_bytes += sent_len;
                    },
                    Err(err) => {
                        error!("stream send_package err={}", err);
                    }
                }
            }
        }

        if sent_bytes > 0 {
            self.write_provider().on_sent(sent_bytes as u64, last_packet_number);
        }

        Ok(())
    } 
}

#[async_trait]
impl StreamProvider for PackageStream {
    fn remote_id(&self) -> IncreaseId {
        self.0.remote_id
    }

    fn local_ep(&self) -> &Endpoint {
        self.0.tunnel.local()
    }

    fn remote_ep(&self) -> &Endpoint {
        self.0.tunnel.remote()
    }

    fn start(&self, owner: &StreamContainer) {
        let stream = self.clone();
        let owner = owner.clone();
        task::spawn(async move {
            loop {
                let now = bucky_time_now();
                let mut packages = Vec::new(); 
                let write_result = stream.write_provider().on_time_escape(&stream, now, &mut packages);
                if write_result.is_err() {
                    owner.break_with_error(write_result.unwrap_err(), true, true);
                    stream.read_provider().break_with_error(BuckyError::new(BuckyErrorCode::ErrorState, "stream broken"));
                    break;
                } 
                let write_result = write_result.unwrap();
                let read_result = stream.read_provider().on_time_escape(&stream, now, &mut packages);
                if write_result.is_err() && read_result.is_err() {
                    owner.on_shutdown(true);
                    break;
                }
                let _ = stream.send_packages(packages);
		stream.drain_delay();
                let _ = future::timeout(stream.config().package.atomic_interval, future::pending::<()>()).await;
            } 
        });
    }

    fn shutdown(&self, which: Shutdown, _owner: &StreamContainer) -> Result<(), std::io::Error> {
        match which {
            Shutdown::Write => {
                let _ = self.write_provider().close(self, None);
            }, 
            Shutdown::Read => {
                let _ = self.read_provider().close();
            }, 
            Shutdown::Both => {
                let _ = self.write_provider().close(self, None);
                let _ = self.read_provider().close();
            }
        }
        Ok(())
    }

    fn clone_as_package_handler(&self) -> Option<Box<dyn OnPackage<SessionData>>> {
        Some(Box::new(self.clone()))
    }

    fn clone_as_provider(&self) -> Box<dyn StreamProvider> {
        Box::new(self.clone())
    }

    fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<usize>> {
        self.read_provider().readable(cx.waker())
    }

    fn poll_read(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        self.read_provider().read(self, cx.waker(), buf)
    }

    fn poll_write(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.write_provider().write(self, cx.waker(), buf)
    }

    fn poll_flush(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.write_provider().flush(cx.waker())
    }

    fn poll_close(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.write_provider().close(self, Some(cx.waker()))
    }
}

impl OnPackage<SessionData> for PackageStream {
    fn on_package(&self, session_data: &SessionData, _: Option<()>) -> BuckyResult<OnPackageResult> {
        let mut packages = Vec::new();
        let r = if session_data.is_syn_ack() {
            let ack_ack = SessionData::new();
            packages.push(DynamicPackage::from(ack_ack));
            OnPackageResult::Handled
        } else {
            trace!("{} on session data {}", self, session_data);
            let write_result = if session_data.is_reset() {
                self.write_provider().reset(self);
                OnPackageResult::Continue
            } else {
                self.write_provider().on_package(session_data, (self, &mut packages))?
            };
            
            match write_result {
                OnPackageResult::Continue => self.read_provider().on_package(session_data, (self, &mut packages))?, 
                OnPackageResult::Handled => OnPackageResult::Handled, 
                _ => unreachable!()
            }
        };

    	{
            let mut pacer = self.0.pacer.lock().unwrap();
            pacer.update(self.write_provider().rate());
        }

        let _ = self.send_packages(packages);

        Ok(r)
    }
}

