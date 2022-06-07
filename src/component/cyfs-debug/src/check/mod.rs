#[cfg(feature = "check")]
mod reentry_checked_mutex;
//mod checked_mutex;

mod tracker;
pub use tracker::*;

#[cfg(not(feature = "check"))]
mod mutex;

#[cfg(feature = "check")]
pub use reentry_checked_mutex::ReenterCheckedMutex as Mutex;

#[cfg(feature = "check")]
pub use reentry_checked_mutex::*;


#[cfg(not(feature = "check"))]
pub use mutex::*;


mod dead;
pub use dead::*;