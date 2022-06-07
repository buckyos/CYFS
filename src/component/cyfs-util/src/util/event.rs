use cyfs_base::BuckyResult;
use async_std::prelude::*;
use async_trait::async_trait;

use std::sync::{Arc, Mutex};

#[async_trait]
pub trait EventListenerAsyncRoutine<P, R>: Send + Sync + 'static
where
    P: Send + Sync + 'static,
    R: 'static,
{
    async fn call(&self, param: &P) -> BuckyResult<R>;
}

#[async_trait]
impl<F, Fut, P, R> EventListenerAsyncRoutine<P, R> for F
where
    P: Send + Sync + 'static,
    R: 'static,
    F: Send + Sync + 'static + Fn(&P) -> Fut,
    Fut: Future<Output = BuckyResult<R>> + Send + 'static,
{
    async fn call(&self, param: &P) -> BuckyResult<R> {
        (self)(param).await
    }
}

#[async_trait]
pub trait EventListenerSyncRoutine<P, R>: Send + Sync + 'static
where
    P: Send + Sync + 'static,
    R: 'static,
{
    fn call(&self, param: &P) -> BuckyResult<R>;
}

#[async_trait]
impl<F, P, R> EventListenerSyncRoutine<P, R> for F
where
    P: Send + Sync + 'static,
    R: 'static,
    F: Send + Sync + 'static + Fn(&P) -> BuckyResult<R>,
{
    fn call(&self, param: &P) -> BuckyResult<R> {
        (self)(param)
    }
}

pub struct SyncEventManager<P, R>
where
    P: Send + Sync + 'static,
    R: 'static,
{
    next_cookie: u32,
    listeners: Vec<(u32, Box<dyn EventListenerSyncRoutine<P, R>>)>,
}

impl<P, R> SyncEventManager<P, R>
where
    P: Send + Sync + 'static,
    R: 'static,
{
    pub fn new() -> Self {
        Self {
            next_cookie: 1,
            listeners: Vec::new(),
        }
    }

    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }

    pub fn on(&mut self, listener: Box<dyn EventListenerSyncRoutine<P, R>>) -> u32 {
        let cookie = self.next_cookie;
        self.next_cookie += 1;

        self.listeners.push((cookie, listener));

        cookie
    }

    pub fn off(&mut self, cookie: u32) -> bool {
        let ret = self.listeners.iter().enumerate().find(|v| v.1 .0 == cookie);

        match ret {
            Some((index, _)) => {
                self.listeners.remove(index);
                true
            }
            None => false,
        }
    }

    pub fn emit(&self, param: &P) -> BuckyResult<Option<R>> {
        let mut ret = None;
        for item in &self.listeners {
            ret = Some(item.1.call(param)?);
        }

        Ok(ret)
    }
}

#[derive(Clone)]
pub struct SyncEventManagerSync<P, R>(Arc<Mutex<SyncEventManager<P, R>>>)
where
    P: Send + Sync + 'static,
    R: 'static;

impl<P, R> SyncEventManagerSync<P, R>
where
    P: Send + Sync + 'static,
    R: 'static,
{
    pub fn new() -> Self {
        let inner = SyncEventManager::new();
        Self(Arc::new(Mutex::new(inner)))
    }

    pub fn listener_count(&self) -> usize {
        self.0.lock().unwrap().listeners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.lock().unwrap().is_empty()
    }

    pub fn on(&self, listener: Box<dyn EventListenerSyncRoutine<P, R>>) -> u32 {
        self.0.lock().unwrap().on(listener)
    }

    pub fn off(&self, cookie: u32) -> bool {
        self.0.lock().unwrap().off(cookie)
    }

    pub fn emit(&self, param: &P) -> BuckyResult<Option<R>> {
        self.0.lock().unwrap().emit(param)
    }
}
