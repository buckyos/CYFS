use named_lock::{NamedLock, NamedLockGuard};

extern crate lazy_static;
use lazy_static::lazy_static;
use std::sync::Mutex;

pub struct ProcessMutex {
    // Compatibility with older versions is in /tmp/
    prev_lock: NamedLock,

    named_lock: NamedLock,
}

impl ProcessMutex {
    pub fn new(service_name: &str) -> ProcessMutex {
        let name = format!("cyfs_plock_{}", service_name);
        let named_lock;
        #[cfg(unix)]
        {
            let locks_folder = crate::get_cyfs_root_path().join("run/locks");
            if !locks_folder.is_dir() {
                if let Err(e) = std::fs::create_dir_all(&locks_folder) {
                    error!(
                        "create run/locks folder error! folder={}, err={}",
                        locks_folder.display(),
                        e
                    );
                }
            }

            let lock_file = locks_folder.join(&format!("{}.lock", name));
            named_lock = NamedLock::with_path(lock_file).unwrap();
        }
        #[cfg(not(unix))]
        {
            named_lock = NamedLock::create(&name).unwrap();
        }

        Self {
            prev_lock: NamedLock::create(&name).unwrap(),
            named_lock,
        }
    }

    pub fn acquire(&self) -> Option<NamedLockGuard> {
        match self.prev_lock.try_lock() {
            Ok(_guard) => {
                // do nothing
            }
            Err(_) => {
                // old proc old must be holded by other process!
                return None;
            }
        }

        match self.named_lock.try_lock() {
            Ok(guard) => {
                Some(guard)
            }

            Err(_e) => {
                None
            }
        }
    }
}

pub(crate) struct ServiceName(String);

impl ServiceName {
    pub fn new() -> ServiceName {
        ServiceName("".to_owned())
    }

    pub fn init(&mut self, service_name: &str) {

        // 可能被多次初始化，但每次初始化name后都会马上使用
        assert!(self.0.is_empty());
        self.0 = service_name.to_owned();
    }

    pub fn detach(&mut self) -> String {
        self.0.split_off(0)
    }
}

// 保持对guard的引用，避免释放
static mut LOCK_GUARD_HOLDER: Option<NamedLockGuard<'static>> = None;

lazy_static! {
    pub(crate) static ref SERVICE_NAME: Mutex<ServiceName> = {
        return Mutex::new(ServiceName::new());
    };

    pub(crate) static ref CURRENT_PROCESS_MUTEX: ProcessMutex = {
        let name = SERVICE_NAME.lock().unwrap().detach();
        assert!(!name.is_empty());

        ProcessMutex::new(&name)
    };

    pub(crate) static ref CURRENT_PROC_LOCK: bool = {
        let guard = CURRENT_PROCESS_MUTEX.acquire();
        if guard.is_none() {
            false
        } else {
            unsafe {
                LOCK_GUARD_HOLDER = guard;
            }
            
            true
        }
    };
}