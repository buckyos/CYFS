use super::path::PATHS;
use super::reload_system_config;
use crate::config::DEVICE_CONFIG_MANAGER;
use crate::daemon::Daemon;
use cyfs_base::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct SystemConfigMonitor {
    watcher: Arc<Option<notify::RecommendedWatcher>>,
    daemon: Daemon,
}

impl SystemConfigMonitor {
    fn new(daemon: Daemon) -> Self {
        let watcher = Self::start_monitor(daemon.clone()).ok();

        Self::start_periodic_check(daemon.clone());
        Self {
            watcher: Arc::new(watcher),
            daemon,
        }
    }

    pub fn start(daemon: Daemon) {
        use once_cell::sync::OnceCell;

        static INIT: OnceCell<SystemConfigMonitor> = OnceCell::new();
        INIT.get_or_init(|| Self::new(daemon));
    }

    async fn on_changed(daemon: &Daemon) -> BuckyResult<()> {
        let changed = reload_system_config().await?;
        if !changed {
            return Ok(());
        }

        let _ = DEVICE_CONFIG_MANAGER.init_repo();
        
        DEVICE_CONFIG_MANAGER.get_repo().clear_cache().await;

        daemon.wakeup_check_update();

        Ok(())
    }

    fn start_periodic_check(daemon: Daemon) {
        async_std::task::spawn(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(60 * 60)).await;
                let _ = Self::on_changed(&daemon).await;
            }
        });
    }

    fn start_monitor(daemon: Daemon) -> BuckyResult<notify::RecommendedWatcher> {
        use notify::{RecursiveMode, Watcher};

        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(event) => {
                info!("got system config file event: {:?}", event);

                if PATHS.system_config.is_file() {
                    let daemon = daemon.clone();
                    async_std::task::spawn(async move {
                        let _ = Self::on_changed(&daemon).await;
                    });
                }
            }
            Err(e) => error!("watch system config file error: {:?}", e),
        })
        .map_err(|e| {
            let msg = format!("create system-config.toml monitor failed! {}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::NotSupport, msg)
        })?;

        watcher
            .watch(&PATHS.system_config, RecursiveMode::NonRecursive)
            .map_err(|e| {
                let msg = format!(
                    "watch system-config.toml monitor failed! file={}, {}",
                    PATHS.system_config.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::Failed, msg)
            })?;

        info!(
            "start monitor system-config file change! file={}",
            PATHS.system_config.display()
        );

        Ok(watcher)
    }
}
