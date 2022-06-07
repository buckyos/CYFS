use cyfs_base::{BuckyError, BuckyResult};
use ood_daemon::{ServiceMode, DEVICE_CONFIG_MANAGER, SERVICE_MANAGER, init_system_config};
use super::asset::InstallTarget;


use std::path::PathBuf;

pub struct DaemonEnv {
    target: InstallTarget,
}

impl DaemonEnv {
    pub fn new(target: &InstallTarget) -> Self {
        Self {
            target: target.to_owned(),
        }
    }

    pub async fn prepare(&self) -> BuckyResult<()> {
        init_system_config().await?;

        DEVICE_CONFIG_MANAGER.init().await?;

        DEVICE_CONFIG_MANAGER.fetch_config().await?;

        Ok(())
    }
}

pub struct OodDaemonInit {
    pub service_dir: Option<PathBuf>,
}

impl OodDaemonInit {
    pub fn new() -> OodDaemonInit {
        OodDaemonInit { service_dir: None }
    }

    pub async fn init(&mut self) -> BuckyResult<()> {
        
        {
            SERVICE_MANAGER.change_mode(ServiceMode::Installer);

            // 禁止删除旧安装包
            SERVICE_MANAGER.enable_gc(false);
        }

        {
            // 同步并加载最新的device_config.cfg
            if let Err(e) = DEVICE_CONFIG_MANAGER.load_and_apply_config().await {
                let msg = format!("load device config failed! err={}", e);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        Ok(())
    }

    pub fn start(&mut self) -> BuckyResult<()> {
        match SERVICE_MANAGER.get_service_info("ood-daemon") {
            Some(service_info) => {
                info!(
                    "init ood-daemon success! fid={}, version={}",
                    service_info.config.fid, service_info.config.version
                );
                let service = service_info.service.as_ref().unwrap();
                service.direct_start_ood_daemon();
                self.service_dir = Some(service.current());
            }
            None => {
                let msg = format!("ood-daemon service not found!");
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        Ok(())
    }
}