use super::gateway_monitor::GATEWAY_MONITOR;
use crate::config::{init_system_config, DeviceConfigManager};
use crate::service::ServiceMode;
use crate::service::SERVICE_MANAGER;
use cyfs_base::{bucky_time_now, BuckyResult};
use cyfs_util::*;
use ood_control::OOD_CONTROLLER;

use async_std::task;
use futures::future::{AbortHandle, Abortable};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// 绑定事件通知
#[derive(Clone)]
struct BindNotify {
    abort_handle: Arc<Mutex<Option<AbortHandle>>>,
}

impl EventListenerSyncRoutine<(), ()> for BindNotify {
    fn call(&self, _: &()) -> BuckyResult<()> {
        if let Some(abort_handle) = self.abort_handle.lock().unwrap().take() {
            info!("wakeup daemon on bind");
            abort_handle.abort();
        }
        Ok(())
    }
}

struct ActionActive {
    update: AtomicU64,
    state: AtomicU64,
}

impl Default for ActionActive {
    fn default() -> Self {
        Self {
            update: AtomicU64::new(0),
            state: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
pub struct Daemon {
    mode: ServiceMode,
    device_config_manager: Arc<DeviceConfigManager>,
    no_monitor: bool,
    last_active: Arc<ActionActive>,
}

impl Daemon {
    // add code here
    pub fn new(mode: ServiceMode, no_monitor: bool) -> Self {
        let device_config_manager = DeviceConfigManager::new();

        Self {
            mode,
            device_config_manager: Arc::new(device_config_manager),
            no_monitor,
            last_active: Arc::new(ActionActive::default()),
        }
    }

    pub async fn run(&self) -> BuckyResult<()> {
        init_system_config().await?;

        self.device_config_manager.init().await?;

        // 关注绑定事件
        let notify = BindNotify {
            abort_handle: Arc::new(Mutex::new(None)),
        };
        OOD_CONTROLLER.bind_event().on(Box::new(notify.clone()));

        let _ = GATEWAY_MONITOR.init().await;

        self.start_check_active();

        self.start_check_service_state();

        self.run_check_update(notify).await;

        Ok(())
    }

    async fn run_check_update(&self, notify: BindNotify) {
        let mut need_load_config = true;
        loop {
            self.last_active
                .update
                .store(bucky_time_now(), Ordering::SeqCst);

            // 记录当前的fid
            let daemon_fid = SERVICE_MANAGER
                .get_service_info(::cyfs_base::OOD_DAEMON_NAME)
                .map(|v| v.config.fid.clone());

            // 首先尝试下载同步配置
            match self.device_config_manager.fetch_config().await {
                Ok(changed) => {
                    if changed {
                        need_load_config = true;
                    } else {
                        // 这里不能设置为false，因为可能load_config处于失败状态，需要等待重试
                    }
                }
                Err(e) => {
                    error!("sync config failed! {}", e);
                }
            }

            if need_load_config {
                if let Err(e) = self.device_config_manager.load_and_apply_config().await {
                    // 加载配置失败，那么需要等下一个周期继续尝试load
                    error!("load config failed! now will retry on next loop! {}", e);
                } else {
                    // 加载配置成功，重置need_load_config
                    info!("load config success!");
                    need_load_config = false;
                }
            } else {
                // 如果没有更新成功device_config,或者device_config没有改变，那么尝试检测一次本地包状态
                SERVICE_MANAGER.sync_service_packages().await;
            }

            // 检查ood-daemon是否发生改变
            let new_daemon_fid = SERVICE_MANAGER
                .get_service_info(::cyfs_base::OOD_DAEMON_NAME)
                .map(|v| v.config.fid.clone());

            // vood模式下，暂不重启ood-daemon
            if self.mode != ServiceMode::VOOD
                && daemon_fid.is_some()
                && new_daemon_fid.is_some()
                && new_daemon_fid != daemon_fid
            {
                info!(
                    "ood-daemon fid changed: {:?} -> {:?}",
                    daemon_fid, new_daemon_fid
                );

                // 需要确保ood-daemon-monitor已经启动
                if !self.no_monitor {
                    use crate::monitor::ServiceMonitor;
                    if ServiceMonitor::launch_monitor().is_ok() {
                        task::sleep(Duration::from_secs(5)).await;
                        std::process::exit(0);
                    }
                } else {
                    std::process::exit(0);
                }
            }

            // 检查绑定状态
            let timer = task::sleep(Duration::from_secs(60 * 10));
            if OOD_CONTROLLER.is_bind() {
                timer.await;
            } else {
                let (abort_handle, abort_registration) = AbortHandle::new_pair();
                *notify.abort_handle.lock().unwrap() = Some(abort_handle);

                match Abortable::new(timer, abort_registration).await {
                    Ok(_) => {
                        debug!("check loop wait timeout, now will check once");
                    }
                    Err(futures::future::Aborted { .. }) => {
                        info!("check loop waked up, now will check once");
                    }
                }
            }
        }
    }

    fn start_check_service_state(&self) {
        let last_active = self.last_active.clone();
        task::spawn(async move {
            loop {
                last_active.state.store(bucky_time_now(), Ordering::SeqCst);

                SERVICE_MANAGER.sync_all_service_state().await;

                task::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    fn start_check_active(&self) {
        const ACTIVE_TIMEOUT: u64 = 1000 * 1000 * 60 * 30;

        let this = self.clone();
        task::spawn(async move {
            loop {
                task::sleep(Duration::from_secs(60)).await;

                let now = bucky_time_now();
                if now - this.last_active.update.load(Ordering::SeqCst) > ACTIVE_TIMEOUT
                    || now - this.last_active.state.load(Ordering::SeqCst) > ACTIVE_TIMEOUT
                {
                    error!("last active timeout! now will exit process!");

                    if !this.no_monitor {
                        use crate::monitor::ServiceMonitor;
                        if ServiceMonitor::launch_monitor().is_ok() {
                            task::sleep(Duration::from_secs(5)).await;
                            std::process::exit(1);
                        }
                    } else {
                        std::process::exit(1);
                    }
                }
            }
        });
    }
}
