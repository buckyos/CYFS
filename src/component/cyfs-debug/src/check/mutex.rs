#[allow(dead_code)]
#[macro_export]
macro_rules! lock {
    ($m:expr) => {
        $m.lock()
    };
}

#[allow(dead_code)]
#[macro_export]
macro_rules! try_lock {
    ($m:expr) => {
        $m.try_lock()
    };
}


pub use std::sync::Mutex;