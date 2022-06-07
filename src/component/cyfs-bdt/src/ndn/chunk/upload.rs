use std::{
    sync::RwLock, 
    collections::LinkedList
};
use async_std::{
    sync::Arc
};
use cyfs_base::*;
use super::super::{
    scheduler::*, 
    channel::UploadSession, 
    channel::PieceSessionType,
};
use super::{
    encode::*,
    view::{ChunkView}
};

struct UploaderImpl {
    view: ChunkView,  
    resource: ResourceManager,
    // 所有channel应当共享raptor encoder 
    // raptor_encoder: RaptorEncoder, 
    sessions: RwLock<LinkedList<UploadSession>>, 
}


// Chunk粒度的所有上传任务；不同channel上的所有session；channel上同chunk应当只有唯一session
// TODO: 这里应当有子 scheduler实现， 在session粒度调度上传
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
        owner: ResourceManager
    ) -> Self {
        Self(Arc::new(UploaderImpl {
            view, 
            resource: ResourceManager::new(Some(owner)), 
            sessions: RwLock::new(LinkedList::new()), 
            // raptor_encoder
        }))
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.view.chunk()
    }

    pub fn resource(&self) -> &ResourceManager {
        &self.0.resource
    }

    pub fn schedule_state(&self) -> TaskState {
        let sessions = self.0.sessions.read().unwrap();
        Self::collect_state(&*sessions)
    }

    fn collect_state(sessions: &LinkedList<UploadSession>) -> TaskState {
        for session in sessions {
            let state = session.schedule_state();
            match state {
                TaskState::Finished => continue, 
                TaskState::Canceled(_) => continue, 
                _ => {
                    return TaskState::Running(0);
                }
            }
        }
        TaskState::Finished
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
            // TODO: 根据资源管理器调度是不是开始
            // TODO: 根据session type创建 encoder
            
        }
        let encoder = match *session.piece_type() {
            PieceSessionType::RaptorA(_) | PieceSessionType::RaptorB(_) => TypedChunkEncoder::Raptor(self.0.view.raptor_encoder()),
            PieceSessionType::Stream(_) => TypedChunkEncoder::Range(RangeEncoder::from_reader(self.0.view.reader().unwrap(), self.chunk())), 
            _ => unreachable!()
        };
        session.start(encoder);

        Ok(())
    }
}

impl Scheduler for ChunkUploader {
    fn collect_resource_usage(&self) {
        let mut sessions = self.0.sessions.write().unwrap();
        let mut remain = LinkedList::new();
        loop {
            if let Some(session) = sessions.pop_front() {
                let state = session.schedule_state();
                match state {
                    TaskState::Finished => {
                        let _ = self.resource().remove_child(session.resource());
                        info!("{} remove session {} for finished", self, session);
                    },  
                    TaskState::Canceled(_) => {
                        let _ = self.resource().remove_child(session.resource());
                        info!("{} remove session {} for canceled", self, session);
                    }, 
                    _ => {
                        remain.push_back(session);
                    }
                }
            } else {
                break;
            }
        }
        *sessions = remain;
    }

    fn schedule_resource(&self) {

    }

    fn apply_scheduled_resource(&self) {
        
    }
}