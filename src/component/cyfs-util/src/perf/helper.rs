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
    perf: Option<&'p PerfHolder>,

    // used on drop
    err: BuckyErrorCode,
    bytes: Option<u32>,
}

impl<'a, 'b, 'p> PerfScopeRequest<'a, 'b, 'p> {
    pub fn new(
        perf: &'p PerfHolder,
        id: &'a str,
        key: &'b str,
        err: BuckyErrorCode,
        bytes: Option<u32>,
    ) -> Self {
        if let Some(perf) = perf.get() {
            perf.begin_request(id, key);
        }

        Self {
            perf: Some(perf),
            id,
            key,
            err,
            bytes,
        }
    }

    pub fn end(mut self, err: BuckyErrorCode, bytes: Option<u32>) {
        if let Some(perf) = self.perf.take().unwrap().get() {
            perf.end_request(self.id, &self.key, err, bytes);
        }
    }
}

impl<'a, 'b, 'p> Drop for PerfScopeRequest<'a, 'b, 'p> {
    fn drop(&mut self) {
        if let Some(perf) = self.perf {
            if let Some(perf) = perf.get() {
                perf.end_request(self.id, self.key, self.err, self.bytes);
            }
        }
    }
}

pub struct PerfScopeRequestWithOwnedKey<'a, 'p> {
    id: &'a str,
    key: String,
    perf: Option<&'p PerfHolder>,

    // used on drop
    err: BuckyErrorCode,
    bytes: Option<u32>,
}

impl<'a, 'p> PerfScopeRequestWithOwnedKey<'a, 'p> {
    pub fn new(
        perf: &'p PerfHolder,
        id: &'a str,
        key: impl Into<String>,
        err: BuckyErrorCode,
        bytes: Option<u32>,
    ) -> Self {
        let key = key.into();
        if let Some(perf) = perf.get() {
            perf.begin_request(id, &key);
        }

        Self {
            perf: Some(perf),
            id,
            key,
            err,
            bytes,
        }
    }

    pub fn end(mut self, err: BuckyErrorCode, bytes: Option<u32>) {
        if let Some(perf) = self.perf.take().unwrap().get() {
            perf.end_request(self.id, &self.key, err, bytes);
        }
    }
}

impl<'a, 'p> Drop for PerfScopeRequestWithOwnedKey<'a, 'p> {
    fn drop(&mut self) {
        if let Some(perf) = self.perf {
            if let Some(perf) = perf.get() {
                perf.end_request(self.id, &self.key, self.err, self.bytes);
            }
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
    ($perf:expr, $id:expr) => {
        if let Some(perf) = $perf.get() {
            perf.begin_request($id, "");
        }
    };
}

#[macro_export]
macro_rules! perf_end_request {
    ($perf:expr, $id:expr) => {
        if let Some(perf) = $perf.get() {
            perf.end_request($id, "", BuckyErrorCode::Ok, None);
        }
    };
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
        let __perf = PerfScopeRequestWithOwnedKey::new(
            &$perf,
            $id,
            perf_request_unique_id(),
            BuckyErrorCode::Ok,
            None,
        );
    };
    ($perf:expr, $id:expr, $key:expr) => {
        let __perf = PerfScopeRequestWithOwnedKey::new(&$perf, $id, $key, BuckyErrorCode::Ok, None);
    };
}

#[macro_export]
macro_rules! perf_scope {
    ($perf:expr, $id:expr, $key:expr, $codes:block) => {{
        let __perf = $perf.get();
        if let Some(perf) = &__perf {
            perf.begin_request($id, $key);
        }

        match ($codes) {
            Ok(r) => {
                if let Some(perf) = &__perf {
                    perf.end_request($id, $key, BuckyErrorCode::Ok, None);
                }
                Ok(r)
            }
            Err(e) => {
                if let Some(perf) = &__perf {
                    perf.end_request($id, $key, e.code(), None);
                }
                Err(e)
            }
        }
    }};

    ($perf:expr, $id:expr, $codes:block) => {
        perf_scope!($perf, $id, "", $codes)
    }
}

#[macro_export]
macro_rules! perf_rev_scope_request {
    ($perf:expr, $id:expr) => {
        let __perf = PerfScopeRequestWithOwnedKey::new(
            &$perf,
            $id,
            perf_request_unique_id(),
            BuckyErrorCode::Failed,
            None,
        );
    };
    ($perf:expr, $id:expr, $key:expr) => {
        let __perf =
            PerfScopeRequestWithOwnedKey::new(&$perf, $id, $key, BuckyErrorCode::Failed, None);
    };
    ($perf:expr, $id:expr, $key:expr, $err:expr) => {
        let __perf = PerfScopeRequestWithOwnedKey::new(&$perf, $id, $key, $err, None);
    };
}

#[macro_export]
macro_rules! perf_end_scope_request {
    ($val:expr, $err:expr, $bytes:expr) => {
        // let __perf = concat_idents!("__", "perf");
        $val.end($err, $bytes)
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
    ($perf:expr, $id:expr, $err:expr, $name:expr, $value:expr) => {
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
