use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

lazy_static::lazy_static! {
    static ref TIMER: Mutex<timer::Timer> = {
        warn!("tracker timer manager launched...");
        Mutex::new(timer::Timer::new())
    };
}

#[derive(Clone, Eq, PartialEq)]
pub enum TimeoutTrackerCategory {
    EnterLock,
    Lock,
    Scope,
}

impl fmt::Display for TimeoutTrackerCategory {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let v = match *self {
            TimeoutTrackerCategory::EnterLock => "enter_lock",
            TimeoutTrackerCategory::Lock => "lock",
            TimeoutTrackerCategory::Scope => "scope",
        };

        write!(fmt, "{}", v)
    }
}

pub struct TimeoutTracker {
    categroy: TimeoutTrackerCategory,
    name: String,
    timer: timer::Guard,
}

impl TimeoutTracker {
    pub fn new_scope(name: &str, dur: Duration) -> Self {
        Self::new(TimeoutTrackerCategory::Scope, name, dur)
    }

    pub fn new(categroy: TimeoutTrackerCategory, name: impl Into<String>, dur: Duration) -> Self {
        let categroy1 = categroy.clone();
        let name = name.into();
        let dur = chrono::Duration::from_std(dur).unwrap();
        let name1 = name.clone();
        let timer = TIMER
            .lock()
            .unwrap()
            .schedule_with_delay(dur.clone(), move || {
                error!(
                    "tracker during extend limit: categroy={}, name={}, dur={}",
                    categroy1, name1, dur
                );
            });

        Self {
            categroy,
            name,
            timer,
        }
    }
}

impl Drop for TimeoutTracker {
    fn drop(&mut self) {
        #[cfg(feature = "trace")]
        trace!("tracker leave: category={}, name={}",self.categroy,self.name);
    }
}

#[allow(dead_code)]
#[cfg(feature = "check")]
#[macro_export]
macro_rules! scope_tracker {
    ($dur:expr) => {
        let tracker =
            cyfs_debug::TimeoutTracker::new_scope(&format!("{}:{}", file!(), line!()), $dur);
    };
}

#[allow(dead_code)]
#[cfg(not(feature = "check"))]
#[macro_export]
macro_rules! scope_tracker {
    ($dur:expr) => {};
}
