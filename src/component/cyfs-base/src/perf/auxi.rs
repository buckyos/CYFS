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
    ( $id:expr, $key:expr, $err:expr, $bytes:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.end_request($id, $key, $err, $bytes);
        }
    }
}

#[macro_export]
macro_rules! perf_acc {
    ( $id:expr, $err:expr, $size:expr ) => {
        if let Some(ins) = crate::get_current_perf_instance() {
            ins.acc($id, $err, $size);
        }
    }
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