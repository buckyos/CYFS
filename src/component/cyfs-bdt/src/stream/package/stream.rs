use log::*;
use std::{
    time::Duration, 
    task::{Context, Poll}, 
};
use async_std::{
    sync::Arc,
    task, 
    future
};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::*, 
    interface,
    tunnel::{udp::Tunnel as UdpTunnel, tunnel::Tunnel}, 
    cc
};
use super::super::{
    container::StreamContainer, 
    stream_provider::{Shutdown, StreamProvider}};
use super::{
    write::WriteProvider,  
    read::ReadProvider
};

#[derive(Clone)]
pub struct Config {
    pub connect_resend_interval: Duration, 
    pub atomic_interval: Duration, 
    pub break_overtime: Duration,  
    pub msl: Duration, 
    pub cc: cc::Config
}

struct PackageStreamImpl {
    config: super::super::container::Config, 
    owner_disp: String, 
    tunnel: UdpTunnel, 
    local_id: IncreaseId,
    remote_id: IncreaseId, 
    write_provider: WriteProvider, 
    read_provider: ReadProvider
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
        owner: &super::super::container::StreamContainerImpl, 
        local_id: IncreaseId, 
        remote_id: IncreaseId,
    ) -> BuckyResult<Self> {
        let owner_disp = format!("{}", owner);
        let config = owner.tunnel().stack().config().stream.stream.clone();

        let write_provider = WriteProvider::new(&config);
        let read_provider = ReadProvider::new(&config);
        let stream = Self(Arc::new(PackageStreamImpl {
            owner_disp, 
            config, 
            tunnel: owner.tunnel().default_udp_tunnel()?, 
            local_id, 
            remote_id, 
            write_provider,
            read_provider
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
        
        for package in packages {
            let _ = self.0.tunnel.send_package(package);
        }
        Ok(())
    } 
}

#[async_trait]
impl StreamProvider for PackageStream {
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
                    owner.as_ref().break_with_error(&owner, write_result.unwrap_err());
                    stream.read_provider().break_with_error(BuckyError::new(BuckyErrorCode::ErrorState, "stream broken"));
                    break;
                } 
                let write_result = write_result.unwrap();
                let read_result = stream.read_provider().on_time_escape(&stream, now, &mut packages);
                if write_result.is_err() && read_result.is_err() {
                    owner.as_ref().on_shutdown(&owner);
                    break;
                }
                let _ = stream.send_packages(packages);
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
                
            }, 
            Shutdown::Both => {
                let _ = self.write_provider().close(self, None);
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
    fn on_package(&self, session_data: &SessionData, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        let mut packages = Vec::new();
        let r = if session_data.is_syn_ack() {
            let ack_ack = SessionData::new();
            packages.push(DynamicPackage::from(ack_ack));
            Ok(OnPackageResult::Handled)
        } else {
            trace!("{} on session data {}", self, session_data);
            match self.write_provider().on_package(session_data, (self, &mut packages))? {
                OnPackageResult::Continue => self.read_provider().on_package(session_data, (self, &mut packages)), 
                OnPackageResult::Handled => Ok(OnPackageResult::Handled), 
                _ => unreachable!()
            }
        }?;
        let _ = self.send_packages(packages);
        Ok(r)
    }
}

