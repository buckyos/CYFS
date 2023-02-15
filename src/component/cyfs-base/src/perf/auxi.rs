#[macro_export]
macro_rules! declare_module_perf_isolate {
    ( $id:expr ) => {
        pub const PERF_ISOLATE_ID: &str = $id;
        pub fn get_current_perf_instance() -> Option<&'static cyfs_base::PerfIsolateRef> {
            static CURRENT_PERF_INSTANCE: once_cell::sync::OnceCell<cyfs_base::PerfIsolateRef> = once_cell::sync::OnceCell::new();
            match CURRENT_PERF_INSTANCE.get() {
                Some(ins) => Some(ins),
                None => {
                    match cyfs_base::PERF_MANGER.get() {
                        Some(manager) => {
                            let ins = manager.get_isolate(PERF_ISOLATE_ID);
                            let _ = CURRENT_PERF_INSTANCE.set(ins);
                            CURRENT_PERF_INSTANCE.get()
                        }
                        None => {
                            None
                        }
                    }
                }
            }
        }

        pub struct PerfScopeRequest<'a> {
            id: &'a str,
            key: String,
        }
        
        impl<'a> PerfScopeRequest<'a> {
            pub fn new(
                id: &'a str,
            ) -> Self {
                let key = cyfs_base::perf_request_unique_id();
                if let Some(ins) = crate::get_current_perf_instance() {
                    ins.begin_request(id, &key);
                }
        
                Self {
                    id,
                    key,
                }
            }
        }
        
        impl<'a> Drop for PerfScopeRequest<'a> {
            fn drop(&mut self) {
                if let Some(ins) = crate::get_current_perf_instance() {
                    ins.end_request(self.id, &self.key, cyfs_base::BuckyErrorCode::Ok, None);
                }
            }
        }
    }
}

use std::sync::atomic::{AtomicU64, Ordering};

pub fn perf_request_unique_id() -> String {
    static INDEX: AtomicU64 = AtomicU64::new(0);
    let ret = INDEX.fetch_add(1, Ordering::SeqCst);
    ret.to_string()
}

#[macro_export]
macro_rules! perf_scope_request {
    ( $id:expr, $block:block ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            let req_id = cyfs_base::perf_request_unique_id();
            ins.begin_request($id, &req_id);
            match $block {
                Ok(v) => {
                    ins.end_request($id, &req_id, cyfs_base::BuckyErrorCode::Ok, None);
                    Ok(v)
                }
                Err(e) => {
                    ins.end_request($id, &req_id, e.code(), None);
                    Err(e)
                }
            }
        } else {
            $block
        }
    }
}

#[macro_export]
macro_rules! perf_begin_request {
    ( $id:expr, $key:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.begin_request($id, $key);
        }
    }
}

#[macro_export]
macro_rules! perf_end_request {
    ( $id:expr, $key:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.end_request($id, $key, cyfs_base::BuckyErrorCode::Ok, None);
        }
    };
    ( $id:expr, $key:expr, $err:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.end_request($id, $key, $err, None);
        }
    };
    ( $id:expr, $key:expr, $err:expr, $bytes:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.end_request($id, $key, $err, $bytes);
        }
    };
}


#[macro_export]
macro_rules! perf_simple_scope_request {
    ( $id:expr ) => {
        let __perf_simple_scope_request = crate::PerfScopeRequest::new($id);
    }
}


#[macro_export]
macro_rules! perf_acc {
    ($id:expr) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.acc($id, cyfs_base::BuckyErrorCode::Ok, None);
        }
    };
    ($id:expr, $err:expr) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.acc($id, $err, None);
        }
    };
    ( $id:expr, $err:expr, $size:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.acc($id, $err, $size);
        }
    };
}

#[macro_export]
macro_rules! perf_action {
    ( $id:expr, $err:expr, $name:expr, $value:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.action($id, $err, $name, $value);
        }
    }
}

#[macro_export]
macro_rules! perf_record {
    ( $id:expr, $total:expr, $total_size:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.record($id, $total, $total_size);
        }
    }
}