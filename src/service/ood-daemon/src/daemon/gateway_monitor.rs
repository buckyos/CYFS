use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;
use ood_control::OOD_CONTROLLER;

use once_cell::sync::OnceCell;
use std::sync::Mutex;

#[derive(Clone)]
struct ZoneRoleChangedNotify {}

#[async_trait::async_trait]
impl
    EventListenerAsyncRoutine<
        RouterEventZoneRoleChangedEventRequest,
        RouterEventZoneRoleChangedEventResult,
    > for ZoneRoleChangedNotify
{
    async fn call(
        &self,
        param: &RouterEventZoneRoleChangedEventRequest,
    ) -> BuckyResult<RouterEventZoneRoleChangedEventResult> {
        warn!("recv zone role changed notify! {}", param);

        async_std::task::spawn(async move {
            let _ = GATEWAY_MONITOR.sync_zone_role().await;
        });

        let resp = RouterEventResponse {
            call_next: true,
            handled: true,
            response: None,
        };

        Ok(resp)
    }
}

pub struct GatewayMonitor {
    dec_id: ObjectId,
    stack: OnceCell<SharedCyfsStack>,
    zone_role: Mutex<ZoneRole>,
}

impl GatewayMonitor {
    pub fn new() -> Self {
        Self {
            dec_id: cyfs_core::get_system_dec_app().object_id().to_owned(),
            stack: OnceCell::new(),
            zone_role: Mutex::new(ZoneRole::ActiveOOD),
        }
    }

    pub async fn init(&self) -> BuckyResult<()> {
        let stack = SharedCyfsStack::open_default_with_ws_event(Some(self.dec_id.clone())).await?;

        if let Err(_) = self.stack.set(stack.clone()) {
            unreachable!();
        }

        async_std::task::spawn(async move {
            Self::run(stack).await;
        });

        info!("init gateway monitor success!");

        Ok(())
    }

    async fn run(stack: SharedCyfsStack) {
        loop {
            if OOD_CONTROLLER.is_bind() {
                break;
            }

            async_std::task::sleep(std::time::Duration::from_secs(30)).await;
        }

        // wait the gateway startup
        let _ = stack.wait_online(None).await;

        Self::start_monitor();

        if let Err(e) = stack.router_events().add_event(
            "ood-daemon-zone-role-monitor",
            0,
            Box::new(ZoneRoleChangedNotify {}),
        ) {
            error!("add zone role monitor event failed! {}", e);
        }
    }

    pub fn zone_role(&self) -> ZoneRole {
        self.zone_role.lock().unwrap().clone()
    }

    fn start_monitor() {
        async_std::task::spawn(async move {
            Self::run_monitor().await;
        });
    }

    async fn run_monitor() {
        loop {
            if let Err(e) = GATEWAY_MONITOR.sync_zone_role().await {
                error!("sync zone role from gateway error! {}", e);
            }

            async_std::task::sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    async fn sync_zone_role(&self) -> BuckyResult<bool> {
        let stack = self.stack.get().unwrap();

        let req = UtilGetDeviceStaticInfoOutputRequest::new();
        let info = stack.util().get_device_static_info(req).await?.info;

        let changed;
        {
            let mut current = self.zone_role.lock().unwrap();
            if *current != info.zone_role {
                warn!("zone role changed! {} -> {}", current, info.zone_role);
                *current = info.zone_role;
                changed = true;
            } else {
                changed = false;
            }
        }

        if changed {
            // TODO monitor daemon sync service state immediately
        }

        Ok(changed)
    }
}

lazy_static::lazy_static! {
    pub static ref GATEWAY_MONITOR: GatewayMonitor = {
        GatewayMonitor::new()
    };
}
