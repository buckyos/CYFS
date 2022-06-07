use super::super::protocol::*;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;
use cyfs_bdt::{DeviceCache, StackGuard};

use http_types::{Method, Request, StatusCode, Url};
use std::collections::HashMap;
use std::sync::Arc;

struct RequestorState {
    latest_state: Option<SyncZoneRequest>,
    during: bool,
}

impl RequestorState {
    fn new() -> Self {
        Self {
            latest_state: None,
            during: false,
        }
    }
}
pub(super) struct SyncServerRequestor {
    requestor: Box<dyn HttpRequestor>,
    service_url: Url,

    zone_request_state: Mutex<RequestorState>,
}

impl SyncServerRequestor {
    pub fn new(requestor: Box<dyn HttpRequestor>) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/sync/", addr);
        let service_url = Url::parse(&url).unwrap();

        Self {
            requestor,
            service_url,
            zone_request_state: Mutex::new(RequestorState::new()),
        }
    }

    fn encode_zone_request(&self, req: &SyncZoneRequest) -> Request {
        let url = self.service_url.join("zone").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        http_req.set_content_type(::tide::http::mime::JSON);
        http_req.set_body(req.encode_string());

        http_req
    }

    pub async fn zone_update(&self, req: SyncZoneRequest) {
        // 检查状态，如果在同步中了，那么直接pending并返回
        {
            let mut state = self.zone_request_state.lock().unwrap();
            state.latest_state = Some(req);
            if state.during {
                debug!(
                    "sync zone update during: state={:?}, device={}",
                    state.latest_state.as_ref().unwrap(),
                    self.requestor.remote_addr()
                );
                return;
            }
        }

        let mut retry_interval = 5;
        let mut count = 0;
        loop {
            let req;
            {
                let mut state = self.zone_request_state.lock().unwrap();
                let cur_req = state.latest_state.take();
                if cur_req.is_none() {
                    state.during = false;
                    break;
                } else {
                    state.during = true;
                    req = cur_req.unwrap();
                }
            }
            match self.zone_update_impl(&req).await {
                Ok(_) => {
                    retry_interval = 5;
                    count = 0;

                    continue;
                }
                Err(e) => {
                    error!(
                        "sync zone update error, now will retry after {} secs: device={}, {}",
                        retry_interval,
                        self.requestor.remote_addr(),
                        e
                    );

                    {
                        // 同步失败后，如果没有pending的latest_req，那么需要把当前失败的恢复回去，等待下次重试
                        let mut state = self.zone_request_state.lock().unwrap();
                        if state.latest_state.is_none() {
                            state.latest_state = Some(req);
                        }
                    }

                    async_std::task::sleep(std::time::Duration::from_secs(retry_interval)).await;

                    retry_interval *= 2;
                    count += 1;

                    // 超出最大重试次数，device可能下线了，终止此次zone_update操作
                    if count > 3 {
                        error!(
                            "sync zone update max retry count, now will break! device={}",
                            self.requestor.remote_addr(),
                        );

                        self.zone_request_state.lock().unwrap().during = false;
                        break;
                    }
                }
            }
        }
    }

    pub async fn zone_update_impl(&self, req: &SyncZoneRequest) -> BuckyResult<()> {
        let http_req = self.encode_zone_request(req);

        debug!(
            "sync will update zone: target={}, root={}",
            self.requestor.remote_addr(),
            req.zone_root_state,
        );

        let mut resp = self
            .requestor
            .request_timeout(http_req, std::time::Duration::from_secs(30))
            .await?;

        match resp.status() {
            StatusCode::Ok => {
                info!(
                    "sync zone update success: device={}, root={}",
                    self.requestor.remote_addr(),
                    req.zone_root_state,
                );

                Ok(())
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "ping resp failed: root={}, device={}, status={} err={}",
                    req.zone_root_state,
                    self.requestor.remote_addr(),
                    code,
                    e
                );

                Err(e)
            }
        }
    }
}

pub(crate) struct SyncServerRequestorManager {
    bdt_stack: StackGuard,
    device_manager: Box<dyn DeviceCache>,

    ood_sync_vport: u16,
    requestors: Arc<Mutex<HashMap<DeviceId, Arc<SyncServerRequestor>>>>,
}

impl SyncServerRequestorManager {
    pub fn new(
        bdt_stack: StackGuard,
        device_manager: Box<dyn DeviceCache>,
        ood_sync_vport: u16,
    ) -> Self {
        assert!(ood_sync_vport > 0);

        Self {
            bdt_stack,
            device_manager,
            ood_sync_vport,
            requestors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn zone_update(
        &self,
        device_list: &Vec<DeviceId>,
        req: SyncZoneRequest,
    ) -> BuckyResult<()> {
        for device_id in device_list {
            match self.get_requestor(device_id).await {
                Ok(requestor) => {
                    let req = req.clone();
                    async_std::task::spawn(async move {
                        requestor.zone_update(req).await;
                    });
                }
                Err(e) => {
                    error!("get requestor for device failed: {}, {}", device_id, e);
                }
            }
        }

        Ok(())
    }

    async fn get_requestor(&self, device_id: &DeviceId) -> BuckyResult<Arc<SyncServerRequestor>> {
        {
            let list = self.requestors.lock().unwrap();
            if let Some(requestor) = list.get(device_id) {
                return Ok(requestor.clone());
            }
        }

        let requestor = self.init_requestor(device_id).await?;
        let requestor = Arc::new(requestor);

        let mut list = self.requestors.lock().unwrap();
        list.insert(device_id.to_owned(), requestor.clone());

        Ok(requestor)
    }

    async fn init_requestor(&self, device_id: &DeviceId) -> BuckyResult<SyncServerRequestor> {
        let device = self.device_manager.search(device_id).await?;

        let bdt_requestor =
            BdtHttpRequestor::new(self.bdt_stack.clone(), device, self.ood_sync_vport);
        let requestor = SyncServerRequestor::new(Box::new(bdt_requestor));

        info!(
            "init sync server bdt requestor to device={} success!",
            device_id
        );

        Ok(requestor)
    }
}
