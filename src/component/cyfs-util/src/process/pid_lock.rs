use cyfs_base::*;

use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use fs2::FileExt;

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

#[cfg(windows)]
use super::win_process::Process;


#[derive(Debug, PartialEq)]
pub enum PidLockError {
    LockExists,
    InvalidState,
}

type PidLockResult = Result<(), PidLockError>;

#[derive(Debug, PartialEq)]
enum PidlockState {
    New,
    Acquired,
    Released,
}

fn getpid() -> u32 {
    unsafe { libc::getpid() as u32 }
}

fn process_exists(pid: i32) -> bool {
    // From the POSIX standard: If sig is 0 (the null signal), error checking
    // is performed but no signal is actually sent. The null signal can be
    // used to check the validity of pid.
    #[cfg(not(windows))]
    unsafe {
        let result = libc::kill(pid, 0);
        result == 0
    }

    #[cfg(windows)]
    {
        match Process::open(pid as u32) {
            Ok(_) => true,
            Err(_e) => false,
        }
    }
}

fn kill_process(pid: i32) -> bool {
    #[cfg(not(windows))]
    unsafe {
        let result = libc::kill(pid, 9);
        result == 0
    }

    #[cfg(windows)]
    {
        let ret = Process::open(pid as u32);
        if let Err(e) = ret {
            error!("open process for kill failed! pid={}, err={}", pid, e);
            return false;
        }

        let proc = ret.unwrap();
        match proc.kill() {
            Ok(_) => true,
            Err(_e) => false,
        }
    }
}

pub struct PidLock {
    pid: u32,
    pub old_pid: u32,
    path: PathBuf,
    state: PidlockState,

    pid_file: Option<File>,
}

impl PidLock {
    pub fn new(path: &Path) -> Self {
        PidLock {
            pid: getpid(),
            old_pid: 0u32,
            path: path.to_owned(),
            state: PidlockState::New,
            pid_file: None,
        }
    }

    pub fn check(&mut self) -> u32 {
        assert!(self.old_pid == 0);

        self.check_stale();

        return self.old_pid;
    }

    // 检查当前pid文件里面的path是否和fid匹配
    pub fn check_fid(&self, fid: &str) -> BuckyResult<bool> {
        debug!("will check fid {}", fid);

        let fid = fid.to_owned();

        match fs::OpenOptions::new().read(true).open(self.path.as_path()) {
            Ok(mut file) => {
                let mut contents = String::new();
                if let Err(e) = file.read_to_string(&mut contents) {
                    error!("read file error: {}", e);
                    return Err(e.into());
                }

                let info: Vec<&str> = contents.trim().split("|").collect();

                if info.len() < 2 {
                    let msg = format!("invalid pid file format: {}", contents);
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                } else {
                    let path_str = info[1].to_owned();
                    match path_str.find(&fid) {
                        Some(_) => {
                            debug!("fid found in exe path! fid={}, path={}", fid, path_str);
                            Ok(true)
                        }
                        None => {
                            warn!("fid not found in exe path! fid={}, path={}", fid, path_str);
                            Ok(false)
                        }
                    }
                }
            }
            Err(e) => {
                error!("open pid file error! file={} ,err={}", self.path.display(), e);
                Err(e.into())
            }
        }
    }

    fn check_stale(&mut self) {
        debug!("will check_stale: {}", self.path.display());

        match fs::OpenOptions::new().read(true).open(self.path.as_path()) {
            Ok(mut file) => {
                let mut contents = String::new();
                if let Err(e) = file.read_to_string(&mut contents) {
                    error!("read file error: {}", e);
                    return;
                }

                let info: Vec<&str> = contents.trim().split("|").collect();

                match info[0].parse::<i32>() {
                    Ok(pid) => {
                        if !process_exists(pid) {
                            warn!("old process {} not exists! removing stale pid file at {}", pid, self.path.display());
                            if let Err(e) = fs::remove_file(&self.path) {
                                error!("remove file error: {}", e);
                            }
                        } else {
                            info!("old process exists: {}, {}", pid, self.path.display());
                            self.old_pid = pid as u32;
                        }
                    }
                    Err(e) => {
                        error!("parse old process pid error! {} value={:?}, err={}", self.path.display(), info, e);
                        if let Err(e) = fs::remove_file(&self.path) {
                            error!("remove file error: {}", e);
                        }
                    }
                }
            }
            Err(_) => {}
        };
    }

    pub fn kill(&self) -> bool {
        assert!(self.old_pid > 0);

        info!("will kill process: {} {}", self.path.display(), self.old_pid);

        kill_process(self.old_pid as i32)
    }

    pub fn acquire(&mut self, ignore_exists: bool) -> PidLockResult {
        match self.state {
            PidlockState::New => {}
            _ => {
                return Err(PidLockError::InvalidState);
            }
        }
        self.check_stale();

        if ignore_exists {
            if self.path.exists() {
                if let Err(e) = fs::remove_file(self.path.as_path()) {
                    error!("remove old pid file error: {} {}", self.path.display(), e);
                }
            }
        }

        assert!(self.pid_file.is_none());

        let ret;
        #[cfg(windows)]
        {
            ret = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .share_mode(winapi::um::winnt::FILE_SHARE_READ)
            .open(self.path.as_path());
        } 
        #[cfg(not(windows))]
        {
            ret = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(self.path.as_path());
        }

        let mut file = match ret
        {
            Ok(file) => file,
            Err(e) => {
                info!("acquire pid lock failed! file={} err={}", self.path.display(), e);
                return Err(PidLockError::LockExists);
            }
        };

        // 以pid|path格式写入
        let path = match std::env::current_exe() {
            Ok(v) => v.to_str().unwrap().to_owned(),
            Err(e) => {
                error!("get current_exe failed! err={}", e);
                "".to_owned()
            }
        };

        let content = format!("{}|{}", self.pid, path);
        file.write_all(&content.into_bytes()[..])
            .unwrap();

        if let Err(e) = file.lock_shared() {
            error!("lock pid file error! file={}, err={}", self.path.display(), e);
            return Err(PidLockError::InvalidState);
        }

        self.pid_file = Some(file);
        
        self.state = PidlockState::Acquired;
        Ok(())
    }

    pub fn release(&mut self) -> PidLockResult {
        match self.state {
            PidlockState::Acquired => {}
            _ => {
                return Err(PidLockError::InvalidState);
            }
        }

        fs::remove_file(self.path.as_path()).unwrap();

        self.state = PidlockState::Released;
        Ok(())
    }
}
