use cyfs_base::BuckyError;

use std::path::PathBuf;
use sysinfo::{Process, ProcessExt, RefreshKind, System, SystemExt};

struct Monitor {
    name: String,
    service_root: PathBuf,

    // 当前的版本，使用fid来唯一区分
    version: String,
}

impl Monitor {
    pub fn new(service_name: &str) -> Self {
        let file_path = ::cyfs_util::get_cyfs_root_path();
        let service_root = file_path.join("services").join(service_name);

        Self {
            name: service_name.to_owned(),
            service_root,
            version: "".to_owned(),
        }
    }

    pub fn check_once(&self) {
        let kind = RefreshKind::new().with_processes();
        let mut system = System::new_with_specifics(kind);
        system.refresh_processes();

        for (pid, process) in system.get_processes() {
            if self.check_process(&process) {
                return;
            }
            println!("{} {}", pid, process.name());
        }
    }

    fn check_process(&self, process: &Process) -> bool {
        let root = process.root();
        if !root.starts_with(&self.service_root) {
            return false;
        }

        if process.pid() as u32 == std::process::id() {
            return false;
        }

        for cmd in process.cmd() {
            if cmd == "--as-monitor" {
                return false;
            }
        }

        info!(
            "monitor service running: {} {}",
            root.display(),
            process.pid()
        );

        true
    }
}
