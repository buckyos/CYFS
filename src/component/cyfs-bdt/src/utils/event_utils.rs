
use std::{sync::{Mutex}, collections::{BTreeSet}};

use cyfs_base::*;
use crate::{
    stack::{Stack}, 
};
use crate::ndn::{
    DefaultNdnEventHandler, 
    NdnEventHandler,
    channel::{
        protocol::v0::*, 
        Channel, 
        DownloadSession
    }, 
};
pub struct RedirectHandle {
    redirect_target: DeviceId,
}

#[async_trait::async_trait]
impl NdnEventHandler for RedirectHandle {
    async fn on_newly_interest(&self, _stack: &Stack, interest: &Interest, from: &Channel) -> BuckyResult<()> {
        let resp_interest = 
            RespInterest { session_id: interest.session_id.clone(), 
                           chunk: interest.chunk.clone(), 
                           err: BuckyErrorCode::Redirect,
                           redirect: Some(self.redirect_target.clone()),
                           redirect_referer: Some(String::default()),
                           to: None,
                        };
        from.resp_interest(resp_interest);

        Ok(())

    }

    fn on_unknown_piece_data(&self, _stack: &Stack, _piece: &PieceData, _from: &Channel) -> BuckyResult<DownloadSession> {
        Err(BuckyError::new(BuckyErrorCode::Interrupted, "no session downloading"))
    }

}

pub struct ForwardEventHandle {
    target: DeviceId,
    default_handle: DefaultNdnEventHandler,
    session_cache: Mutex<BTreeSet<u32>>
}

impl ForwardEventHandle {
    pub fn new(target: DeviceId) -> Self {
        Self { 
            target,
            default_handle: DefaultNdnEventHandler::new(),
            session_cache: Mutex::new(BTreeSet::new()),
        }
    }
}

#[async_trait::async_trait]
impl NdnEventHandler for ForwardEventHandle {
    async fn on_newly_interest(&self, stack: &Stack, interest: &Interest, from: &Channel) -> BuckyResult<()> {
        if !self.target.eq(from.remote()) {
            if let Some(target) = stack.ndn().channel_manager().channel_of(&self.target) {
                {
                    let session_cache = &mut *self.session_cache.lock().unwrap();

                    if session_cache.contains(&interest.session_id.value()) {
                        return Ok(());
                    }

                    session_cache.insert(interest.session_id.value());
                }

                let interest = 
                    Interest { session_id: interest.session_id.clone(), 
                               chunk: interest.chunk.clone(),
                               prefer_type: interest.prefer_type.clone(),
                               from: Some(from.remote().clone()),
                               referer: Some(String::default()),};

                target.interest(interest);
                Ok(())
                
            } else {
                from.resp_interest(RespInterest {
                    session_id: interest.session_id.clone(),
                    chunk: interest.chunk.clone(), 
                    err: BuckyErrorCode::ConnectionAborted,
                    redirect: None,
                    redirect_referer: None,
                    to: None
                });
                Ok(())
            }
        } else {
            self.default_handle.on_newly_interest(stack, interest, from).await
        }
    }

    fn on_unknown_piece_data(&self, _stack: &Stack, _piece: &PieceData, _from: &Channel) -> BuckyResult<DownloadSession> {
        Err(BuckyError::new(BuckyErrorCode::Interrupted, "no session downloading"))
    }
}
