use cyfs_base::BuckyResult;

use super::pid_lock::{PidLock, PidLockError};

pub struct ProcessLock {
    service_name: String,
    pid_lock: PidLock,
}

impl ProcessLock {
    pub fn new(service_name: &str) -> ProcessLock {
        let pid_folder = crate::get_cyfs_root_path().join("run");
        if !pid_folder.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&pid_folder) {
                error!(
                    "create pid folder error! folder={}, err={}",
                    pid_folder.display(),
                    e
                );
            }
        }
        let name = format!("{}.pid", service_name);
        let pid_file = pid_folder.join(name);
        ProcessLock {
            service_name: service_name.to_owned(),
            pid_lock: PidLock::new(pid_file.as_path()),
        }
    }

    pub fn get_old_pid(&self) -> u32 {
        self.pid_lock.old_pid
    }

    pub fn check(&mut self) -> u32 {
        self.pid_lock.check()
    }

    pub fn check_fid(&mut self, fid: &str) -> BuckyResult<bool> {
        self.pid_lock.check_fid(fid)
    }

    pub fn acquire(&mut self) -> Result<(), PidLockError> {
        self.pid_lock.acquire(false)
    }

    // 忽略已经存在的pid文件和检测
    pub fn force_acquire(&mut self) -> Result<(), PidLockError> {
        self.pid_lock.acquire(true)
    }

    pub fn release(&mut self) -> Result<(), PidLockError> {
        self.pid_lock.release()
    }

    pub fn kill(&self) -> bool {
        self.pid_lock.kill()
    }
}
