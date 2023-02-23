use super::path::PATHS;
use super::reload_system_config;
use cyfs_base::*;



pub struct SystemConfigMonitor {
    watcher: Option<notify::RecommendedWatcher>,
}

impl SystemConfigMonitor {
    pub fn start() {
        use once_cell::sync::OnceCell;

        static INIT: OnceCell<SystemConfigMonitor> = OnceCell::new();
        INIT.get_or_init(|| {
            let watcher = Self::start_monitor().ok();
   
            Self::start_periodic_check();
            Self {
                watcher,
            }
        });
    }

    fn start_periodic_check() {
        async_std::task::spawn(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(60 * 15)).await;
                let _ = reload_system_config().await;
            }
        });
    }

    fn start_monitor() -> BuckyResult<notify::RecommendedWatcher> {
        use notify::{RecursiveMode, Watcher};

        let mut watcher = notify::recommended_watcher(|res| match res {
            Ok(event) => {
                info!("got system config file event: {:?}", event);

                if PATHS.system_config.is_file() {
                    async_std::task::spawn(async move {
                        let _ = reload_system_config().await;
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