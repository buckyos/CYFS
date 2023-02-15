use crate::config::ServiceState;
use crate::config::{init_system_config, DEVICE_CONFIG_MANAGER, OOD_DAEMON_SERVICE};
use crate::service::SERVICE_MANAGER;
use cyfs_base::*;

pub struct ServicesStopController {
}

impl ServicesStopController {
    pub fn new() -> Self {
        Self {
        }
    }

    pub async fn stop_all(&self) -> BuckyResult<()> {
        init_system_config().await?;

        DEVICE_CONFIG_MANAGER.init()?;

        let mut list = DEVICE_CONFIG_MANAGER.load_config()?;

        info!("service list: {:?}", list);

        list.iter_mut().for_each(|service| {
            if service.name == OOD_DAEMON_SERVICE {
                // rename ood-daemon service to force stop!
                service.name = format!("{}/", OOD_DAEMON_SERVICE);
            }
            service.target_state = ServiceState::Stop;
        });

        SERVICE_MANAGER.load(list).await?;

        crate::monitor::ServiceMonitor::stop_monitor_process(::cyfs_base::OOD_DAEMON_NAME);

        Ok(())
    }
}
