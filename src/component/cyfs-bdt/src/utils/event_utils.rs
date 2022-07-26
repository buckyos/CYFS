
use std::{sync::{Mutex}, collections::BTreeMap};

use cyfs_base::*;
use crate::{
    stack::{Stack} 
};
use crate::{
    ndn::{NdnEventHandler, DefaultNdnEventHandler}, 
    // ChunkTask,
    // chunk::ChunkDownloadConfig,
    channel::{
        protocol::*, 
        Channel, 
        UploadSession, DownloadSession
    }, 
};

pub struct RedirectHandle {}

#[async_trait::async_trait]
impl NdnEventHandler for RedirectHandle {
    async fn on_newly_interest(&self, stack: &Stack, interest: &Interest, from: &Channel) -> BuckyResult<()> {
        let session = UploadSession::redirect(interest.chunk.clone(), 
                                              interest.session_id.clone(), 
                                              interest.prefer_type.clone(), 
                                              from.clone(), 
                                              DeviceId::default(), 
                                              String::default());

        let _ = from.upload(session.clone());
        session.on_interest(interest)
    }

    fn on_unknown_piece_data(&self, _stack: &Stack, _piece: &PieceData, _from: &Channel) -> BuckyResult<DownloadSession> {
        Err(BuckyError::new(BuckyErrorCode::Interrupted, "no session downloading"))
    }

}

pub struct ForwardEventHandle {
    target: DeviceId,
    default_handle: DefaultNdnEventHandler,
    session_cache: Mutex<BTreeMap<u32, bool>>
}

impl ForwardEventHandle {
    pub fn new(target: DeviceId) -> Self {
        Self { 
            target,
            default_handle: DefaultNdnEventHandler::new(),
            session_cache: Mutex::new(BTreeMap::new()),
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

                    if session_cache.contains_key(&interest.session_id.value()) {
                        return Ok(());
                    }

                    session_cache.insert(interest.session_id.value(), true);
                }

                let forward_session = 
                    UploadSession::forward(interest.chunk.clone(),
                                           interest.session_id.clone(), 
                                           interest.prefer_type.clone(), 
                                           from.clone(),
                                           target,
                                           String::default());
    
                let _ = forward_session.on_interest(interest);
                Ok(())
                
            } else {
                let session = UploadSession::canceled(interest.chunk.clone(), 
                                                                     interest.session_id.clone(),
                                                                     interest.prefer_type.clone(),
                                                                     from.clone(),
                                                                     BuckyErrorCode::ConnectionAborted);
                let _ = from.upload(session.clone());
                session.on_interest(interest)
            }
        } else {
            self.default_handle.on_newly_interest(stack, interest, from).await
        }
    }

    fn on_unknown_piece_data(&self, _stack: &Stack, _piece: &PieceData, _from: &Channel) -> BuckyResult<DownloadSession> {
        Err(BuckyError::new(BuckyErrorCode::Interrupted, "no session downloading"))
    }
}
