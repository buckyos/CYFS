use super::requestor::*;
use cyfs_base::*;
use cyfs_bdt::*;

use http_types::{Request, Response};

#[derive(Clone)]
pub struct BdtHttpRequestor {
    bdt_stack: StackGuard,
    device_id: DeviceId,
    device: Device,
    vport: u16,
}

impl BdtHttpRequestor {
    pub fn new(bdt_stack: StackGuard, device: Device, vport: u16) -> Self {
        Self {
            bdt_stack,
            device_id: device.desc().device_id(),
            device,
            vport,
        }
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
        let build_params = BuildTunnelParams {
            remote_const: self.device.desc().clone(),
            remote_sn: None,
            remote_desc: Some(self.device.clone()),
        };

        let bdt_stream = self
            .bdt_stack
            .stream_manager()
            .connect(self.vport, Vec::new(), build_params)
            .await
            .map_err(|e| {
                let msg = format!(
                    "connect to {} failed! during={}ms, {}",
                    self.remote_addr(),
                    begin.elapsed().as_millis(),
                    e
                );
                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
            })?;

        if let Some(conn_info) = conn_info {
            *conn_info = HttpRequestConnectionInfo::Bdt((
                bdt_stream.local_ep().unwrap(),
                bdt_stream.remote_ep().unwrap(),
            ));
        }

        let seq = bdt_stream.sequence();
        debug!(
            "bdt connect to {} success, seq={:?}, during={}ms",
            self.remote_addr(),
            seq,
            begin.elapsed().as_millis(),
        );
        // bdt_stream.display_ref_count();

        match async_h1::connect(bdt_stream, req.take().unwrap()).await {
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
