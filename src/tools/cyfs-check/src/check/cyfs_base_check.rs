use crate::{CheckCore, CheckType};
use async_trait::async_trait;
use log::*;
use std::path::Path;
use sysinfo::{ProcessExt, ProcessRefreshKind, RefreshKind, System, SystemExt};

pub struct CyfsBaseCheck {}

#[async_trait]
impl CheckCore for CyfsBaseCheck {
    async fn check(&self, check_type: CheckType) -> bool {
        // 检测环境变量
        if let Ok(channel) = std::env::var("CYFS_CHANNEL") {
            warn!("find CYFS_CHANNEL env: {}, ensure?", &channel);
        }
        // 判断是不是OOD，通过检测c:\cyfs目录
        let root = cyfs_util::get_cyfs_root_path();
        if root.exists() {
            info!("Check {} Base Path {} Ok.", check_type, root.display());

            // 检查关键目录，先检查service和etc这两个
            if !root.join("etc").exists() {
                error!("{} etc folder not exists!", check_type);
                return false;
            }
            if !root.join("services").exists() {
                error!("{} services folder not exists!", check_type);
                return false;
            }

            if cyfs_util::get_service_config_dir("desc")
                .join("device.sec")
                .exists()
            {
                info!("{} has activated", check_type);
            } else {
                error!("{} not activated", check_type);
                // 没激活后续就都不检测了
                return false;
            }

            // 检查进程
            let mut sys = System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));
            sys.refresh_processes();
            let check_process_name = match check_type {
                CheckType::OOD => "ood-daemon",
                CheckType::Runtime => "cyfs-runtime",
            };

            let mut find_process = false;
            for (pid, process) in sys.processes() {
                let name = Path::new(process.name())
                    .file_stem()
                    .map(|s| s.to_str().unwrap_or(""))
                    .unwrap_or("");
                if name == check_process_name {
                    info!(
                        "{} core process {} running at {}",
                        check_type, check_process_name, pid
                    );
                    find_process = true;
                    break;
                }
            }

            if !find_process {
                warn!("core process {} not running", check_process_name);
            }
        }

        return true;
    }

    fn name(&self) -> &str {
        "Base Check"
    }
}
