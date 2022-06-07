use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::Duration;

use super::tracker::*;

/*
struct MutexTrackerInner {
    name: String,
    own_thread_id: Option<ThreadId>,
    timer: Option<TimeoutTracker>,
}

impl MutexTrackerInner {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            own_thread_id: None,
            timer: None,
        }
    }

    fn check_owned_and_set_unowned(&mut self) {
        let current = self.own_thread_id.take();
        let _ = current.expect(&format!("mutex shouled be owned: {}", self.name));

        self.own_thread_id = None;
    }

    fn check_unowned_and_set_owned(&mut self) {
        let current = self.own_thread_id.take();
        if let Some(id) = current {
            panic!("mutex should not been owned: {}, owned={:?}", self.name, id);
        }

        self.own_thread_id = Some(std::thread::current().id());
    }

    pub fn enter(&mut self, pos: &str) {
        let full_name = format!("{}:{}", self.name, pos);
        let timer = TimeoutTracker::new(&full_name, Duration::from_secs(10));
    }
}

struct MutexTracker(Arc<Mutex<MutexTrackerInner>>);

impl MutexTracker {
    pub fn new(name: &str) -> Self {
        Self(Arc::new(Mutex::new(MutexTrackerInner::new(name))))
    }

    pub fn enter(pos: &str) {}
}

const SEQ: AtomicU64 = AtomicU64::new(0);

fn get_next_seq() -> u64 {
    SEQ.fetch_add(1, Ordering::SeqCst)
}


*/

pub struct CheckedMutex<T> {
    inner: Mutex<T>,
}

impl<T> CheckedMutex<T> {
    pub fn new(t: T) -> CheckedMutex<T> {
        CheckedMutex {
            inner: Mutex::new(t),
        }
    }
}

type LockResult<'a, T> = Result<CheckedMutexGuard<'a, T>, PoisonError<MutexGuard<'a, T>>>;
type TryLockResult<'a, T> = Result<CheckedMutexGuard<'a, T>, TryLockError<MutexGuard<'a, T>>>;

impl<T> CheckedMutex<T> {
    #[deprecated(note = "Please use cyfs_debug::lock! macro instead")]
    pub fn lock(&self) -> LockResult<'_, T> {
        self.lock_with_pos("")
    }

    #[deprecated(note = "Please use cyfs_debug::try_lock! macro instead")]
    pub fn try_lock(&self) -> TryLockResult<'_, T> {
        self.try_lock_with_pos("")
    }

    pub fn lock_with_pos(&self, pos: &str) -> LockResult<'_, T> {

        #[cfg(feature = "trace")]
        trace!("try enter mutex: {}", pos);

        let _timer = TimeoutTracker::new(
            TimeoutTrackerCategory::EnterLock,
            pos,
            Duration::from_secs(10),
        );
        self.inner.lock().map(|lock| {
            #[cfg(feature = "trace")]
            trace!("enter mutex: {}", pos);

            CheckedMutexGuard::new(pos, lock)
        })
    }

    pub fn try_lock_with_pos(&self, pos: &str) -> TryLockResult<'_, T> {
        #[cfg(feature = "trace")]
        trace!("try enter mutex: {}", pos);

        match self.inner.try_lock() {
            Ok(lock) => {
                #[cfg(feature = "trace")]
                trace!("enter mutex: {}", pos);

                let ret = CheckedMutexGuard::new(pos, lock);

                Ok(ret)
            }
            Err(e) => Err(e),
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.inner.is_poisoned()
    }

    pub fn into_inner(self) -> std::sync::LockResult<T>
    where
        T: Sized,
    {
        self.inner.into_inner()
    }

    pub fn get_mut(&mut self) -> std::sync::LockResult<&mut T> {
        self.inner.get_mut()
    }
}

use std::fmt;
use std::ops::{Deref, DerefMut};

pub struct CheckedMutexGuard<'a, T: ?Sized + 'a> {
    inner: MutexGuard<'a, T>,
    timer: TimeoutTracker,
}

impl<'a, T: ?Sized> CheckedMutexGuard<'a, T> {
    fn new(name: &str, lock: MutexGuard<'a, T>) -> Self {
        let timer =
            TimeoutTracker::new(TimeoutTrackerCategory::Lock, name, Duration::from_secs(10));
        Self { inner: lock, timer }
    }
}

impl<T: ?Sized> Deref for CheckedMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.inner
    }
}

impl<T: ?Sized> DerefMut for CheckedMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.inner
    }
}

impl<T: ?Sized> Drop for CheckedMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {}
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for CheckedMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for CheckedMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[allow(dead_code)]
#[macro_export]
macro_rules! lock {
    ($m:expr) => {
        $m.lock_with_pos(&format!("{}:{}", file!(), line!()))
    };
}

#[allow(dead_code)]
#[macro_export]
macro_rules! try_lock {
    ($m:expr) => {
        $m.try_lock_with_pos(&format!("{}:{}", file!(), line!()))
    };
}
