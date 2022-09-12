use super::source::RequestSourceInfo;

use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

pub struct RequestAccessInfoInner {
    pub source: RequestSourceInfo,
    pub access_verified: AtomicBool,
}

#[derive(Clone)]
pub struct RequestAccessInfo(Arc<RequestAccessInfoInner>);

impl RequestAccessInfo {
    pub fn new(source: RequestSourceInfo) -> Self {
        Self(Arc::new(RequestAccessInfoInner {
            source,
            access_verified: AtomicBool::new(false),
        }))
    }

    pub fn is_verified(&self) -> bool {
        self.0.access_verified.load(Ordering::SeqCst)
    }

    pub fn set_access_verified(&self, verified: bool) -> bool {
        self.0.access_verified.swap(verified, Ordering::SeqCst)
    }
}
