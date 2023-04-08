use super::requestor::*;
use cyfs_base::*;
use cyfs_bdt::*;

use http_types::{Request, Response};
use std::sync::Mutex;
use std::sync::{RwLock, Arc};

struct DeviceConnectWithSNCache {
    devices: Mutex<lru_time_cache::LruCache<DeviceId, ()>>,
}

impl DeviceConnectWithSNCache {
    pub fn new() -> Self {
        Self {
            devices: Mutex::new(lru_time_cache::LruCache::with_expiry_duration_and_capacity(
                std::time::Duration::from_secs(60 * 10),
                256,
            )),
        }
    }

    pub fn try_connect_via_sn(device_id: &DeviceId) -> bool {
        static D: once_cell::sync::OnceCell<DeviceConnectWithSNCache> = once_cell::sync::OnceCell::new();
        D.get_or_init(|| {
            DeviceConnectWithSNCache::new()
        }).try_connect_via_sn_impl(device_id)
    }
    
    fn try_connect_via_sn_impl(&self, device_id: &DeviceId) -> bool {
        let mut list = self.devices.lock().unwrap();

        // force remove expired items
        list.iter();

        if let Some(_) = list.peek(device_id) {
            return false;
        }

        list.insert(device_id.to_owned(), ());

        true
    }
}


#[derive(Clone)]
pub struct BdtHttpRequestor {
    bdt_stack: StackGuard,
    device_id: DeviceId,
    device: Arc<RwLock<Device>>,
    vport: u16,
}

impl BdtHttpRequestor {
    pub fn new(bdt_stack: StackGuard, device: Device, vport: u16) -> Self {
        Self {
            bdt_stack,
            device_id: device.desc().device_id(),
            device: Arc::new(RwLock::new(device)),
            vport,
        }
    }

    pub fn device(&self) -> Device {
        self.device.read().unwrap().clone()
    }

    pub fn update_device(&self, device: Device) {
        assert_eq!(device.desc().device_id(), self.device_id);
        *self.device.write().unwrap() = device;
    }
    
    pub fn has_wan_endpoint(&self) -> bool {
        self.device.read().unwrap().has_wan_endpoint()
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    async fn connect(&self, with_remote_desc: bool) -> BuckyResult<StreamGuard> {
        let begin = std::time::Instant::now();

        let device = self.device();

        let build_params = BuildTunnelParams {
            remote_const: device.desc().clone(),
            remote_sn: None,
            remote_desc: if with_remote_desc {
                Some(device)
            } else {
                None
            },
        };

        let bdt_stream = self
            .bdt_stack
            .stream_manager()
            .connect(self.vport, Vec::new(), build_params)
            .await
            .map_err(|e| {
                let msg = format!(
                    "connect to {} failed! with_desc={}, during={}ms, {}",
                    self.remote_addr(),
                    with_remote_desc,
                    begin.elapsed().as_millis(),
                    e
                );
                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
            })?;

        Ok(bdt_stream)
    }
}

#[async_trait::async_trait]
impl HttpRequestor for BdtHttpRequestor {
    async fn request_ext(
        &self,
        req: &mut Option<Request>,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        debug!(
            "will create bdt stream connection to {}",
            self.remote_addr()
        );

        let begin = std::time::Instant::now();

        let bdt_stream = match self.connect(true).await {
            Ok(stream) => stream,
            Err(e) => {
                if !self.has_wan_endpoint() {
                    return Err(e);
                }
                
                if !DeviceConnectWithSNCache::try_connect_via_sn(&self.device_id) {
                    return Err(e);
                }

                info!("now will retry connect via sn: device={}", self.device_id);
                self.connect(false).await?
            }
        };

        let seq = bdt_stream.sequence();
        if let Some(conn_info) = conn_info {
            let local_addr = bdt_stream.local_ep().ok_or_else(|| {
                let msg = format!("get local_ep from bdt stream but empty! seq={:?}", seq);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::NotConnected, msg)
            })?;
    
            let remote_addr = bdt_stream.remote_ep().ok_or_else(|| {
                let msg = format!("get remote_ep from bdt stream but empty! seq={:?}", seq);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::NotConnected, msg)
            })?;

            *conn_info = HttpRequestConnectionInfo::Bdt((
                local_addr,
                remote_addr,
            ));
        }

        debug!(
            "bdt connect to {} success, seq={:?}, during={}ms",
            self.remote_addr(),
            seq,
            begin.elapsed().as_millis(),
        );
        // bdt_stream.display_ref_count();

        let req = req.take().unwrap();
        let req = self.add_default_headers(req);

        match async_h1::connect(bdt_stream, req).await {
            Ok(resp) => {
                info!(
                    "http-bdt request to {} success! during={}ms, seq={:?}",
                    self.remote_addr(),
                    begin.elapsed().as_millis(),
                    seq,
                );
                Ok(resp)
            }
            Err(e) => {
                let msg = format!(
                    "http-bdt request to {} failed! during={}ms, seq={:?}, {}",
                    self.remote_addr(),
                    begin.elapsed().as_millis(),
                    seq,
                    e,
                );
                error!("{}", msg);
                Err(BuckyError::from(msg))
            }
        }
    }

    fn remote_addr(&self) -> String {
        format!("{}:{}", self.device_id, self.vport)
    }

    fn remote_device(&self) -> Option<DeviceId> {
        Some(self.device_id.clone())
    }

    fn clone_requestor(&self) -> Box<dyn HttpRequestor> {
        Box::new(self.clone())
    }

    async fn stop(&self) {
        // to nothing
    }
}
