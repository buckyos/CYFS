
use std::{
    sync::{RwLock, Arc},
};
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}
};
use super::super::super::{ 
    scheduler::*, 
    channel::*, 
};
use super::super::{
    view::ChunkView, 
};
use super::{
    config::ChunkDownloadConfig
};

struct DoubleSessionImpl {
    download_session: RwLock<Option<DownloadSession>>, 
    src_ref: DeviceId,
    src_owner: DeviceId,
    view: ChunkView,
    chunk_id: ChunkId,
    stack: WeakStack,
    session_id: TempSeq,
    referer: Option<String>,
}

#[derive(Clone)]
pub struct DoubleSession(Arc<DoubleSessionImpl>);

impl DoubleSession {
    pub fn new(
        stack: WeakStack, 
        view: &ChunkView,  
        session_id: TempSeq, 
        config: &ChunkDownloadConfig) -> Self {
        Self(Arc::new(DoubleSessionImpl{
                download_session: RwLock::new(None),
                src_ref: config.prefer_source.clone(),
                src_owner: config.second_source.as_ref().unwrap().clone(),
                view: view.clone(),
                chunk_id: view.chunk().clone(),
                stack: stack.clone(),
                session_id,
                referer: config.referer.clone(),
            }))
    }

    pub fn take_chunk_content(&self) -> Option<Arc<Vec<u8>>> {
        let session = self.0.download_session.read().unwrap();
        if let Some(s) = &*session {
            return s.take_chunk_content();
        }

        None
    }

    pub async fn start(&self) -> TaskState {
        let state = self.start_stream_session(&self.0.src_ref).await;
        match state {
            TaskState::Finished => TaskState::Finished,
            _ => {
                error!("download unfinish, from ref src, state={:?}", state);

                let state = self.start_stream_session(&self.0.src_owner).await;
                match state {
                    TaskState::Finished => TaskState::Finished,
                    state => {
                        error!("download unfinish, src owner src, state={:?}", state);

                        state
                    }
                }
            }
        }
    }

    async fn start_stream_session(&self, src: &DeviceId) -> TaskState {
        let session = self.new_stream_session(src);

        {
            let s = &mut *self.0.download_session.write().unwrap();
            *s = Some(session.clone());
        }

        if let Ok(_) = session.channel().download(session.clone()) {
            session.wait_finish().await
        } else {
            unreachable!()
        }
    }

    fn new_stream_session(&self, device_id: &DeviceId) -> DownloadSession {
        let stack = self.stack();
        let channel = stack.ndn().channel_manager().create_channel(device_id);

        DownloadSession::new(
            self.0.chunk_id.clone(), 
            self.0.session_id, 
            channel, 
            PieceSessionType::Stream(0),
            self.0.view.clone(),
            self.0.referer.clone())
    }

    fn stack(&self) -> Stack {
        Stack::from(&self.0.stack)
    }
}
