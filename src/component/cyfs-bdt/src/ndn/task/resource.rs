
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

pub struct ResourcePerfData {
    progress: AtomicUsize,
    // stat: PerfData,
    stat: AtomicPtr<Box<dyn PerfDataAbstract>>,
}

impl std::clone::Clone for ResourcePerfData {
    fn clone(&self) -> Self {
        let progress = self.progress();
        let stat = Box::into_raw(Box::new(self.stat()));
        Self {
            progress: AtomicUsize::new(progress),
            stat: AtomicPtr::new(stat),
        }
    }
}

impl ResourcePerfData {
    pub fn new() -> Self {
        ResourcePerfData {
            progress: AtomicUsize::new(0),
            stat: AtomicPtr::new(null_mut()),
        }
    }

    #[inline]
    pub fn set_progress(&self, progress: usize) {
        self.progress.store(progress, Ordering::SeqCst);
    }
    #[inline]
    pub fn progress(&self) -> usize {
        self.progress.load(Ordering::SeqCst)
    }
    #[inline]
    pub fn stat(&self) -> Box<dyn PerfDataAbstract> {
        let ptr = self.stat.load(Ordering::SeqCst);
        unsafe {
            if ptr == null_mut() {
                PerfData::default().clone_as_perfdata()
            } else {
                (*ptr).clone_as_perfdata()
            }
        }
    }

    pub fn merge(&self, data: &Box<dyn PerfDataAbstract>) -> &Self {
        let data_cp = Box::into_raw(Box::new(data.clone_as_perfdata()));
        let _ = self.stat.swap(data_cp, Ordering::SeqCst);
        self
    }
}

impl PerfDataAbstract for ResourcePerfData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_as_perfdata(&self) -> Box<dyn PerfDataAbstract> {
        Box::new(self.clone())
    }

    fn byte_total(&self) -> u64 {
        self.stat().byte_total()
    }

    fn bandwidth(&self) -> u64 {
        self.stat().bandwidth()
    }
}

impl std::fmt::Display for ResourcePerfData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stat = self.stat();
        write!(f, "resource per data: progress={}, stat={}",
            self.progress.load(Ordering::SeqCst),
            stat)
    }
}

impl std::default::Default for ResourcePerfData {
    fn default() -> Self {
        Self {
            progress: AtomicUsize::new(0),
            stat: AtomicPtr::new(null_mut()),
        }
    }
}
