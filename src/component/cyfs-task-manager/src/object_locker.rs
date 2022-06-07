use std::sync::{Mutex, Arc};
use std::collections::HashMap;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

struct CDFutureState<RESULT> {
    waker: Option<Waker>,
    result: Option<RESULT>,
}

impl <RESULT> CDFutureState<RESULT> {
    pub fn new() -> Arc<Mutex<CDFutureState<RESULT>>> {
        Arc::new(Mutex::new(CDFutureState {
            waker: None,
            result: None
        }))
    }

    pub fn set_complete(state: &Arc<Mutex<CDFutureState<RESULT>>>, result: RESULT) {
        let mut state = state.lock().unwrap();
        state.result = Some(result);
        if state.waker.is_some() {
            state.waker.take().unwrap().wake();
        }
    }
}

struct CDFuture<RESULT>(Arc<Mutex<CDFutureState<RESULT>>>);

impl <RESULT> CDFuture<RESULT> {
    pub fn new(state: Arc<Mutex<CDFutureState<RESULT>>>) -> Self {
        Self(state)
    }
}

impl <RESULT> Future for CDFuture<RESULT> {
    type Output = RESULT;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.0.lock().unwrap();
        if state.result.is_some() {
            return Poll::Ready(state.result.take().unwrap());
        }

        if state.waker.is_none() {
            state.waker = Some(cx.waker().clone());
        }
        Poll::Pending
    }
}

struct LockerState {
    pub is_locked: bool,
    pub pending_list: Vec<Arc<Mutex<LockerFutureState>>>
}

struct LockerManager {
    locker_map: Mutex<HashMap<String, LockerState>>
}

lazy_static::lazy_static! {
    static ref LOCK_MANAGER: LockerManager = LockerManager::new();
}

type LockerFutureState = CDFutureState<()>;

impl LockerManager {
    pub fn new() -> LockerManager {
        Self {
            locker_map: Mutex::new(HashMap::new())
        }
    }

    pub async fn lock(&self, locker_id: String) {
        let future = {
            let mut locker_map = self.locker_map.lock().unwrap();
            let locker_info = locker_map.get_mut(&locker_id);
            if locker_info.is_none() {
                locker_map.insert(locker_id.clone(), LockerState {
                    is_locked: true,
                    pending_list: Vec::new()
                });
                log::debug!("LockerManager:get locker {}", locker_id);
                return;
            } else {
                let state = locker_info.unwrap();
                if state.is_locked {
                    let future_state = LockerFutureState::new();
                    state.pending_list.push(future_state.clone());
                    let future = CDFuture::new(future_state);
                    future
                } else {
                    state.is_locked = true;
                    log::debug!("LockerManager:get locker {}", locker_id);
                    return;
                }
            }
        };
        log::debug!("LockerManager:waiting locker {}", locker_id);
        future.await;
        log::debug!("LockerManager:get locker {}", locker_id);
    }

    pub fn unlock(&self, locker_id: &str) {
        let mut locker_map = self.locker_map.lock().unwrap();
        let locker_info = locker_map.get_mut(locker_id);
        if locker_info.is_some() {
            let state = locker_info.unwrap();
            if state.pending_list.len() > 0 {
                let future_state = state.pending_list.remove(0);
                LockerFutureState::set_complete(&future_state, ());
            } else {
                state.is_locked = false;
            }
        } else {
            assert!(false);
        }
        log::debug!("LockerManager:free locker {}", locker_id);
    }
}

pub struct Locker {
    locker_id: String,
}

impl Locker {
    pub async fn get_locker(locker_id: String) -> Self {
        LOCK_MANAGER.lock(locker_id.clone()).await;
        Self {
            locker_id
        }
    }
}

impl Drop for Locker {
    fn drop(&mut self) {
        LOCK_MANAGER.unlock(self.locker_id.as_str());
    }
}

pub struct GuardObject<T> {
    locker: Locker,
    obj: T
}

impl <T> GuardObject<T> {
    pub fn new(locker: Locker, obj: T) -> Self {
        Self {
            locker,
            obj
        }
    }

    pub fn release_locker(self) -> T {
        self.obj
    }
}

impl <T> Deref for GuardObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl <T> DerefMut for GuardObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.obj
    }
}
