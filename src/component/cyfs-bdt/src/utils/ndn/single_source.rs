use std::{
    sync::{Arc, RwLock},
    collections::LinkedList
};
use futures::future::{AbortRegistration};
use cyfs_base::*;
use crate::{
    types::*, 
    ndn::{*, channel::DownloadSession}, 
    stack::{Stack}, 
};

enum WaitSession {
    None(StateWaiter), 
    Some(DownloadSession)
}

struct ContextImpl {
    referer: String, 
    source: DownloadSource, 
    session: RwLock<WaitSession>
}

#[derive(Clone)]
pub struct SingleSourceContext(Arc<ContextImpl>);

impl SingleSourceContext {
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn source(&self) -> &DownloadSource {
        &self.0.source
    }

    pub fn from_desc(referer: String, remote: DeviceDesc) -> Self {
        Self(Arc::new(ContextImpl {
            referer, 
            source: DownloadSource {
                target: remote, 
                encode_desc: ChunkEncodeDesc::Stream(None, None, None), 
            }, 
            session: RwLock::new(WaitSession::None(StateWaiter::new()))
        }))
    }

    pub async fn from_id(stack: &Stack, referer: String, remote: DeviceId) -> BuckyResult<Self> {
        let device = stack.device_cache().get(&remote).await
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "device desc not found"))?;
        Ok(Self(Arc::new(ContextImpl {
            referer, 
            source: DownloadSource {
                target: device.desc().clone(), 
                encode_desc: ChunkEncodeDesc::Stream(None, None, None), 
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

    fn source_exists(&self, target: &DeviceId, encode_desc: &ChunkEncodeDesc) -> bool {
        self.source().target.device_id().eq(target) && self.source().encode_desc.support_desc(encode_desc)
    }

    fn sources_of(&self, filter: Box<dyn Fn(&DownloadSource) -> bool>, limit: usize) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        if (*filter)(self.source()) {
            result.push_back(DownloadSource {
                target: self.source().target.clone(), 
                encode_desc: self.source().encode_desc.clone(), 
            });
        } 
        result
    }

    fn on_new_session(&self, new_session: &DownloadSession) {
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
}

