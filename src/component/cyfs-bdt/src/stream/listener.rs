use log::*;
use std::{task::{Context, Poll, Waker}, pin::Pin, sync::{RwLock}, ops::Deref};
use async_std::{sync::{Arc}, channel::{bounded, Sender, Receiver}, task, io::ErrorKind};
pub use futures::future::{Abortable, AbortHandle, Aborted};
use cyfs_base::*;
use super::{StreamManager, WeakStreamManager, StreamGuard};
use cyfs_debug::Mutex;

#[derive(Clone)]
pub struct Config {
    pub backlog: usize
}

pub struct PreAcceptedStream {
    pub stream: StreamGuard, 
    pub question: Vec<u8>
}

pub enum StreamListenerState {
    Listening((Sender<PreAcceptedStream>, Receiver<PreAcceptedStream>)), 
    Stopped
}

struct StreamListenerImpl {
    manager: WeakStreamManager, 
    port: u16, 
    state: RwLock<StreamListenerState>
}

impl std::fmt::Display for StreamListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamListener {{port:{}}}", self.port())
    }
}


#[derive(Clone)]
pub struct StreamListener(Arc<StreamListenerImpl>);

impl StreamListener {
    pub fn new(manager: WeakStreamManager, port: u16, backlog: usize) -> Self {
        Self(Arc::new(StreamListenerImpl {
            manager, 
            port, 
            state: RwLock::new(StreamListenerState::Listening(bounded::<PreAcceptedStream>(backlog)))
        }))
    } 

    pub fn port(&self) -> u16 {
        self.0.port
    }

    pub async fn accept(&self) -> Option<Result<PreAcceptedStream, BuckyError>> {
        // 暂时只有被停止才会失败？
        let receiver = match &*self.0.state.read().unwrap() {
            StreamListenerState::Listening((_, r)) => r.clone(),
            StreamListenerState::Stopped => return None
        };
        match receiver.recv().await {
            Ok(s) => Some(Ok(s)),
            Err(_) => None
        }
    }

    pub fn incoming(&self) -> StreamIncoming<'_> {
        StreamIncoming(self, Arc::new(Mutex::new(IncommingState { result: None, waker: None, is_pending: false })))
    }
    
    pub fn stop(&self) {
        let mut state = self.0.state.write().unwrap();
        if let StreamListenerState::Listening(_) = *state {
            *state = StreamListenerState::Stopped;
        } else {
            return;
        }
        StreamManager::from(&self.0.manager).remove_acceptor(self);
    }

    pub(super) fn push_stream(&self, stream: PreAcceptedStream) {
        let listener = self.clone();
        async_std::task::spawn(async move {
            let s: Option<Sender<PreAcceptedStream>> = match &*listener.0.state.read().unwrap() {
                StreamListenerState::Listening((s, _)) => Some(s.clone()),
                StreamListenerState::Stopped => None
            };
            if let Some(sender) = s {
                if !sender.is_full() {
                    debug!("{} push new pre accepted stream", listener);
                    let _ = sender.send(stream).await;
                } else {
                    debug!("{} backlog full", listener);
                }
            } else {
                debug!("{} not listening", listener);
            }
        });
    }
}


struct IncommingState {
    result: Option<Option<std::io::Result<PreAcceptedStream>>>,
    waker: Option<Waker>,
    is_pending: bool,
}

pub struct StreamIncoming<'a>(&'a StreamListener, Arc<Mutex<IncommingState>>);

impl <'a> async_std::stream::Stream for StreamIncoming<'a> {
    type Item = std::io::Result<PreAcceptedStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<<Self as async_std::stream::Stream>::Item>> {
        let (poll_result, is_accept_next) = {
            let mut last_result = None;
            let mut state = self.1.lock().unwrap();
            std::mem::swap(&mut state.result, &mut last_result);

            let is_pending = state.is_pending;

            match last_result {
                Some(r) => {
                    state.waker = None;
                    assert!(!state.is_pending);
                    const AUTO_ACCEPT_NEXT: bool = true; // 自动再投递一个accept，一直保持一个accept投递
                    state.is_pending = AUTO_ACCEPT_NEXT;
                    (Poll::Ready(r), AUTO_ACCEPT_NEXT)
                }
                None => {
                    state.waker = Some(cx.waker().clone());
                    state.is_pending = true;
                    (Poll::Pending, !is_pending)
                }
            }
        };


        if is_accept_next {
            let acceptor = self.0.clone();
            let next_state = self.1.clone();

            task::spawn(async move {
                let result = match acceptor.accept().await {
                    Some(r) => {
                        Some(r.map_err(|e| std::io::Error::new(ErrorKind::Other, e)))
                    }
                    None => {
                        None
                    }
                };

                let waker = {
                    let mut result = Some(result);
                    let mut next_state = next_state.lock().unwrap();
                    assert!(next_state.is_pending); // 正在accept
                    assert!(next_state.result.is_none()); // 没有结果
                    next_state.is_pending = false;
                    std::mem::swap(&mut next_state.result, &mut result);
                    next_state.waker.clone()
                };

                if let Some(wk) = waker {
                    wk.wake();
                }
            });
        }

        poll_result
    }
}

struct StreamListenerGuardImpl(StreamListener);

impl Drop for StreamListenerGuardImpl {
    fn drop(&mut self) {
        debug!("stream listener guard droped and will stop: port={}", self.0.port());

        self.0.stop()
    }
}

#[derive(Clone)]
pub struct StreamListenerGuard(Arc<StreamListenerGuardImpl>);

impl From<StreamListener> for StreamListenerGuard {
    fn from(a: StreamListener) -> Self {
        Self(Arc::new(StreamListenerGuardImpl(a)))
    }
}

impl std::fmt::Display for StreamListenerGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamListenerGuard {{listener:{}}}", (*self.0).0)
    }
}

impl Deref for StreamListenerGuard {
    type Target = StreamListener;
    fn deref(&self) -> &StreamListener {
        &(*self.0).0
    }
}

impl StreamListenerGuard {
    pub fn incoming(&self) -> StreamIncoming<'_> {
        (&(*self.0).0).incoming()
    }
}
