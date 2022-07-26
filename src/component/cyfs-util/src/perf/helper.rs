use super::holder::*;
use cyfs_base::*;

use std::sync::atomic::{AtomicU64, Ordering};

pub fn perf_request_unique_id() -> String {
    static INDEX: AtomicU64 = AtomicU64::new(0);
    let ret = INDEX.fetch_add(1, Ordering::SeqCst);
    format!("perf_req_{}", ret)
}

pub struct PerfScopeRequest<'a, 'b, 'p> {
    id: &'a str,
    key: &'b str,
    perf: &'p PerfHolderRef,
}

impl<'a, 'b, 'p> PerfScopeRequest<'a, 'b, 'p> {
    pub fn new(perf: &'p PerfHolderRef, id: &'a str, key: &'b str) -> Self {
        if let Some(perf) = perf.get() {
            perf.begin_request(id, key);
        }

        Self { perf, id, key }
    }
}

impl<'a, 'b, 'p> Drop for PerfScopeRequest<'a, 'b, 'p> {
    fn drop(&mut self) {
        if let Some(perf) = self.perf.get() {
            perf.end_request(self.id, self.key, BuckyErrorCode::Ok, None);
        }
    }
}

pub struct PerfScopeRequestWithOwnedKey<'a, 'p> {
    id: &'a str,
    key: String,
    perf: &'p PerfHolderRef,
}

impl<'a, 'p> PerfScopeRequestWithOwnedKey<'a, 'p> {
    pub fn new(perf: &'p PerfHolderRef, id: &'a str, key: impl Into<String>) -> Self {
        let key = key.into();
        if let Some(perf) = perf.get() {
            perf.begin_request(id, &key);
        }

        Self { perf, id, key }
    }
}

impl<'a, 'p> Drop for PerfScopeRequestWithOwnedKey<'a, 'p> {
    fn drop(&mut self) {
        if let Some(perf) = self.perf.get() {
            perf.end_request(self.id, &self.key, BuckyErrorCode::Ok, None);
        }
    }
}

#[macro_export]
macro_rules! perf_request_unique_id {
    () => {
        perf_request_unique_id()
    };
}

#[macro_export]
macro_rules! perf_begin_request {
    ($perf:expr, $id:expr, $key:expr) => {
        if let Some(perf) = $perf.get() {
            perf.begin_request($id, $key);
        }
    };
}

#[macro_export]
macro_rules! perf_end_request {
    ($perf:expr, $id:expr, $key:expr) => {
        if let Some(perf) = $perf.get() {
            perf.end_request($id, $key, BuckyErrorCode::Ok, None);
        }
    };
    ($perf:expr, $id:expr, $key:expr, $err:expr) => {
        if let Some(perf) = $perf.get() {
            perf.end_request($id, $key, $err, None);
        }
    };
    ($perf:expr, $id:expr, $key:expr, $err:expr, $bytes:expr) => {
        if let Some(perf) = $perf.get() {
            perf.end_request($id, $key, $err, Some($bytes));
        }
    };
}

#[macro_export]
macro_rules! perf_scope_request {
    ($perf:expr, $id:expr) => {
        let __perf = PerfScopeRequestWithOwnedKey::new(&$perf, $id, perf_request_unique_id());
    };
    ($perf:expr, $id:expr, $key:expr) => {
        let __perf = PerfScopeRequestWithOwnedKey::new(&$perf, $id, $key);
    };
}

#[macro_export]
macro_rules! perf_acc {
    ($perf:expr, $id:expr) => {
        if let Some(perf) = $perf.get() {
            perf.acc($id, cyfs_base::BuckyErrorCode::Ok, None);
        }
    };
    ($perf:expr, $id:expr, $err:expr) => {
        if let Some(perf) = $perf.get() {
            perf.acc($id, $err, None);
        }
    };
    ($perf:expr, $id:expr, $err:expr, $bytes:expr) => {
        if let Some(perf) = $perf.get() {
            perf.acc($id, $err, Some($bytes));
        }
    };
}

#[macro_export]
macro_rules! perf_action {
    ($perf:expr, $id:ident, $err:ident, $name:ident, $value:ident) => {
        if let Some(perf) = $perf.get() {
            perf.action($id, $err, $name, $value);
        }
    };
}

#[macro_export]
macro_rules! perf_record {
    ($perf:expr, $id:expr, $total:expr) => {
        if let Some(perf) = $perf.get() {
            perf.record($id, $total, None);
        }
    };
    ($perf:expr, $id:expr, $total:expr, $total_size:expr) => {
        if let Some(perf) = $perf.get() {
            perf.record($id, $total, Some($total_size));
        }
    };
}
