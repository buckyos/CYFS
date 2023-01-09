use std::{
    sync::RwLock, 
    collections::LinkedList
};
use async_std::{
    sync::Arc
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::{
    upload::*,
    channel::*, 
};
use super::{
    view::{ChunkView}
};

struct UploaderImpl {
    view: ChunkView,  
    sessions: RwLock<LinkedList<UploadSession>>, 
}


// Chunk粒度的所有上传任务；不同channel上的所有session；channel上同chunk应当只有唯一session
#[derive(Clone)]
pub struct ChunkUploader(Arc<UploaderImpl>);

impl std::fmt::Display for ChunkUploader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkUploader:{{chunk:{}}}", self.chunk())
    }
}

impl ChunkUploader {
    pub fn new(
        view: ChunkView,  
    ) -> Self {
        Self(Arc::new(UploaderImpl {
            view, 
            sessions: RwLock::new(LinkedList::new()), 
            // raptor_encoder
        }))
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.view.chunk()
    }

    pub fn on_schedule(&self, _: Timestamp) -> bool {
        let mut active = false;
        let mut remain = LinkedList::new();
        let mut sessions = self.0.sessions.write().unwrap();

        for session in &*sessions {
            match session.state() {
                UploadTaskState::Uploading(_) => {
                    remain.push_back(session.clone());
                    active = true
                }, 
                _ => {

                }
            }
        }

        std::mem::swap(&mut *sessions, &mut remain);
        active
    }

    pub fn add_session(&self, session: UploadSession) -> BuckyResult<()> {
        info!("{} try add new session {}", self, session);
        {
            let mut sessions = self.0.sessions.write().unwrap();
            if sessions.iter().find(|s| 
                session.channel().remote().eq(s.channel().remote())
                && session.chunk().eq(s.chunk())
                && session.session_id().eq(s.session_id())).is_some() {
                info!("session {} exists in {}, ignore it", session, self);
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "session exists"));
            }
            sessions.push_front(session.clone());
            
        }
        session.start(self.0.view.reader().unwrap());

        Ok(())
    }
}
