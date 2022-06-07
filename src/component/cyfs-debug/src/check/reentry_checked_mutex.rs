use super::tracker::*;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};

pub struct ReenterCheckedMutex<T> {
    inner: Mutex<T>,
    own_thread_id: Arc<Mutex<Option<std::thread::ThreadId>>>,
}

impl<T> ReenterCheckedMutex<T> {
    pub fn new(t: T) -> ReenterCheckedMutex<T> {
        ReenterCheckedMutex {
            inner: Mutex::new(t),
            own_thread_id: Arc::new(Mutex::new(None)),
        }
    }
}

type LockResult<'a, T> = Result<ReenterCheckedMutexGuard<'a, T>, PoisonError<MutexGuard<'a, T>>>;
type TryLockResult<'a, T> = Result<ReenterCheckedMutexGuard<'a, T>, TryLockError<MutexGuard<'a, T>>>;

impl<T> ReenterCheckedMutex<T> {
    pub fn lock(&self) -> LockResult<'_, T> {

        let current = std::thread::current().id();
        // debug!("try enter mutex: {:?}", current);
        {
            let holder = self.own_thread_id.lock().unwrap();
            assert_ne!(*holder, Some(current));
        }

        self.inner.lock().map(|lock| {

            *self.own_thread_id.lock().unwrap() = Some(current);

            ReenterCheckedMutexGuard::new(lock, self.own_thread_id.clone())
        })
    }

    pub fn try_lock(&self) -> TryLockResult<'_, T> {
        let current = std::thread::current().id();

        match self.inner.try_lock() {
            Ok(lock) => {
     
                *self.own_thread_id.lock().unwrap() = Some(current);

                let ret = ReenterCheckedMutexGuard::new(lock, self.own_thread_id.clone());

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

pub struct ReenterCheckedMutexGuard<'a, T: ?Sized + 'a> {
    inner: MutexGuard<'a, T>,
    own_thread_id: Arc<Mutex<Option<std::thread::ThreadId>>>,
    tracker: TimeoutTracker,
}

impl<'a, T: ?Sized> ReenterCheckedMutexGuard<'a, T> {
    fn new(lock: MutexGuard<'a, T>, own_thread_id: Arc<Mutex<Option<std::thread::ThreadId>>>,) -> Self {
        let time =  chrono::Local::now().to_rfc3339();
        let tracker =
            TimeoutTracker::new(TimeoutTrackerCategory::Lock, time, std::time::Duration::from_secs(30));
        Self { inner: lock, own_thread_id, tracker }
    }
}

impl<T: ?Sized> Deref for ReenterCheckedMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.inner
    }
}

impl<T: ?Sized> DerefMut for ReenterCheckedMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.inner
    }
}

impl<T: ?Sized> Drop for ReenterCheckedMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        if let Some(_t) = self.own_thread_id.lock().unwrap().take() {

        } else {
            unreachable!();
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for ReenterCheckedMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for ReenterCheckedMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}


#[allow(dead_code)]
#[macro_export]
macro_rules! lock {
    ($m:expr) => {
        $m.lock()
    };
}

#[allow(dead_code)]
#[macro_export]
macro_rules! try_lock {
    ($m:expr) => {
        $m.try_lock()
    };
}
