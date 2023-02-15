#[macro_export]
macro_rules! declare_module_perf_isolate {
    ( $id:expr ) => {};
}

#[macro_export]
macro_rules! perf_scope_request {
    ( $id:expr, $block:block ) => {
        $block
    };
}

#[macro_export]
macro_rules! perf_begin_request {
    ( $id:expr, $key:expr ) => {};
}

#[macro_export]
macro_rules! perf_end_request {
    ( $id:expr, $key:expr ) => {};
    ( $id:expr, $key:expr, $err:expr ) => {};
    ( $id:expr, $key:expr, $err:expr, $bytes:expr ) => {};
}

#[macro_export]
macro_rules! perf_simple_scope_request {
    ( $id:expr ) => {};
}

#[macro_export]
macro_rules! perf_acc {
    ($id:expr) => {};
    ($id:expr, $err:expr) => {};
    ( $id:expr, $err:expr, $size:expr ) => {};
}

#[macro_export]
macro_rules! perf_action {
    ( $id:expr, $err:expr, $name:expr, $value:expr ) => {};
}

#[macro_export]
macro_rules! perf_record {
    ( $id:expr, $total:expr, $total_size:expr ) => {};
}
