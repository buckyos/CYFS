use std::{
    sync::{Arc, RwLock},
    collections::LinkedList
};
use futures::future::{AbortRegistration};
use cyfs_base::*;
use crate::{
    types::*, 
    ndn::{*, channel::{DownloadSession, DownloadSessionState}}, 
    stack::{Stack}, 
};

enum WaitSession {
    None(StateWaiter), 
    Some(DownloadSession)
}

struct ContextImpl {
    referer: String, 
    create_at: Timestamp, 
    source: DownloadSource<DeviceDesc>, 
    session: RwLock<WaitSession>
}

#[derive(Clone)]
pub struct SingleSourceContext(Arc<ContextImpl>);

impl SingleSourceContext {
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn source(&self) -> &DownloadSource<DeviceDesc> {
        &self.0.source
    }

    pub fn from_desc(referer: String, remote: DeviceDesc) -> Self {
        Self(Arc::new(ContextImpl {
            create_at: bucky_time_now(), 
            referer, 
            source: DownloadSource {
                target: remote, 
                codec_desc: ChunkCodecDesc::Stream(None, None, None), 
            }, 
            session: RwLock::new(WaitSession::None(StateWaiter::new()))
        }))
    }

    pub async fn from_id(stack: &Stack, referer: String, remote: DeviceId) -> BuckyResult<Self> {
        let device = stack.device_cache().get(&remote).await
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "device desc not found"))?;
        Ok(Self(Arc::new(ContextImpl {
            create_at: bucky_time_now(), 
            referer, 
            source: DownloadSource {
                target: device.desc().clone(), 
                codec_desc: ChunkCodecDesc::Stream(None, None, None), 
            },
            session: RwLock::new(WaitSession::None(StateWaiter::new()))
        })))
    }

    pub async fn wait_session(&self, abort: impl futures::Future<Output = BuckyError>) -> BuckyResult<DownloadSession> {
        enum NextStep {
            Wait(AbortRegistration), 
            Some(DownloadSession)
        }

        let next = {
            let mut session = self.0.session.write().unwrap();
            match &mut *session {
                WaitSession::None(waiter) => NextStep::Wait(waiter.new_waiter()), 
                WaitSession::Some(session) => NextStep::Some(session.clone())
            }
        };

        match next {
            NextStep::Some(session) => Ok(session),
            NextStep::Wait(waiter) => StateWaiter::abort_wait(abort, waiter, || {
                let session = self.0.session.read().unwrap();
                match & *session {
                    WaitSession::Some(session) => session.clone(),
                    _ => unreachable!()
                }
            }).await
        }
      
    }
}

#[async_trait::async_trait]
impl DownloadContext for SingleSourceContext {
    fn clone_as_context(&self) -> Box<dyn DownloadContext> {
        Box::new(self.clone())
    }

    fn is_mergable(&self) -> bool {
        false
    }

    fn referer(&self) -> &str {
        self.0.referer.as_str()
    }

    async fn update_at(&self) -> Timestamp {
        self.0.create_at
    }

    async fn sources_of(&self, filter: &DownloadSourceFilter, _limit: usize) -> (LinkedList<DownloadSource<DeviceDesc>>, Timestamp) {
        let mut result = LinkedList::new();
        if filter.check(self.source()) {
            result.push_back(DownloadSource {
                target: self.source().target.clone(), 
                codec_desc: self.source().codec_desc.clone(), 
            });
        } 
        (result, self.0.create_at)
    }

    fn on_new_session(&self, _task: &dyn LeafDownloadTask, new_session: &DownloadSession, _update_at: Timestamp) {
        let waiter = {
            let mut session = self.0.session.write().unwrap();
            match &mut *session {
                WaitSession::None(waiter) => {
                    let waiter = waiter.transfer();
                    *session = WaitSession::Some(new_session.clone());
                    waiter
                } 
                WaitSession::Some(_) => unreachable!()
            }
        };
       
        waiter.wake();
    }

    fn on_drain(
        &self, 
        task: &dyn LeafDownloadTask, 
        _update_at: Timestamp) {
        let session = {
            let session = self.0.session.read().unwrap();
            match &*session {
                WaitSession::Some(session) => Some(session.clone()), 
                _ => None
            }
        };
        
        if let Some(session) = session {
            if let DownloadSessionState::Canceled(err) = session.state() {
                let _ = task.cancel_by_error(err);
            }
        }
    }
}

