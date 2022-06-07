use named_lock::{NamedLock, NamedLockGuard};

extern crate lazy_static;
use lazy_static::lazy_static;
use std::sync::Mutex;

pub struct ProcessMutex {
    named_lock: Option<NamedLock>,
}

impl ProcessMutex {
    pub fn new(service_name: &str) -> ProcessMutex {
        let name = format!("cyfs_plock_{}", service_name);
        ProcessMutex {
            named_lock: Some(NamedLock::create(&name).unwrap()),
        }
    }

    pub fn acquire(&self) -> Option<NamedLockGuard> {
        assert!(self.named_lock.is_some());

        match self.named_lock.as_ref().unwrap().try_lock() {
            Ok(guard) => {
                Some(guard)
            }

            Err(_e) => {
                None
            }
        }
    }

    pub fn release(&mut self) {}
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