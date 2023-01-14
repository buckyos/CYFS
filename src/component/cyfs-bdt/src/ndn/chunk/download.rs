use std::{
    sync::{RwLock, Arc, Weak}, collections::LinkedList,
};
use async_std::{ 
    task, 
};
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}
};
use super::super::{ 
    types::*, 
    channel::*, 
    download::*,
};
use super::{
    cache::*, 
};

#[derive(Debug)]
struct QueryContextOp {
    op_id: IncreaseId, 
    filter: DownloadSourceFilter, 
    limit: usize
}

#[derive(Debug)]
struct StartSessionOp {
    op_id: IncreaseId, 
    update_at: Timestamp, 
    source: DownloadSource<DeviceDesc>
}

#[derive(Debug)]
enum SessionOp {
    None, 
    TrigerDrain(Timestamp), 
    StartSession(StartSessionOp), 
    QueryContext(QueryContextOp)
}

#[derive(Clone)]
enum TryingSession {
    None, 
    Starting(IncreaseId), 
    Running(DownloadSession)
}

impl TryingSession {
    fn as_session(&self) -> Option<&DownloadSession> {
        match self {
            Self::Running(session) => Some(session),
            _ => None
        }
    }
}

struct SingleStreamSession {
    gen_id: IncreaseIdGenerator, 
    update_at: Timestamp, 
    tried: LinkedList<DownloadSession>, 
    trying: TryingSession, 
    querying: Option<IncreaseId>
}

impl SingleStreamSession {
    fn new(update_at: Timestamp) -> (Self, QueryContextOp) {
        let gen_id = IncreaseIdGenerator::new();
        let op_id = gen_id.generate();
        let session = Self {
            gen_id, 
            update_at, 
            tried: LinkedList::new(), 
            trying: TryingSession::None, 
            querying: Some(op_id)
        };
        let op = QueryContextOp {
            op_id, 
            filter: session.next_filter(), 
            limit: 1
        }; 
        (session, op)
    }

    fn check_context(&mut self, update_at: Timestamp) -> SessionOp {
        if self.querying.is_some() {
            return SessionOp::None;
        }

        match self.trying.clone() {
            TryingSession::Starting(_) => SessionOp::None, 
            TryingSession::None => {
                if update_at != self.update_at {
                    let op_id = self.gen_id.generate();
                    let op = QueryContextOp {
                        op_id, 
                        filter: self.next_filter(), 
                        limit: 1
                    }; 
                    self.querying = Some(op_id);
                    SessionOp::QueryContext(op)
                } else {
                    SessionOp::None
                }
            }, 
            TryingSession::Running(session) => {
                match session.state() {
                    DownloadSessionState::Downloading => {
                        if update_at != self.update_at {
                            let op_id = self.gen_id.generate();
                            let op = QueryContextOp {
                                op_id, 
                                filter: self.check_filter(), 
                                limit: 1
                            }; 
                            self.querying = Some(op_id);
                            SessionOp::QueryContext(op)
                        } else {
                            SessionOp::None
                        }
                    }, 
                    DownloadSessionState::Canceled(_) => {
                        self.trying = TryingSession::None;
                        self.tried.push_back(session);
                        let op_id = self.gen_id.generate();
                        let op = QueryContextOp {
                            op_id, 
                            filter: self.next_filter(), 
                            limit: 1
                        }; 
                        self.querying = Some(op_id);
                        SessionOp::QueryContext(op)
                    },
                    DownloadSessionState::Finished => SessionOp::None
                }
            }
        } 
    }

    fn trying(&self) -> Option<&DownloadSession> {
        self.trying.as_session()
    }

    fn next_filter(&self) -> DownloadSourceFilter {
        DownloadSourceFilter {
            exclude_target: Some(self.tried.iter().map(|session| session.source().target.clone()).collect()), 
            include_target: None, 
            include_codec: Some(vec![ChunkCodecDesc::Stream(None, None, None)]), 
        }
    }

    fn check_filter(&self) -> DownloadSourceFilter {
        DownloadSourceFilter {
            exclude_target: Some(self.tried.iter().map(|session| session.source().target.clone()).collect()), 
            include_target: self.trying.as_session().map(|session| vec![session.source().target.clone()]), 
            include_codec: self.trying.as_session().map(|session| vec![session.source().codec_desc.clone()]), 
        }
    }

    fn on_session_created(&mut self, op_id: IncreaseId, session: DownloadSession) -> bool {
        let start = match &self.trying {
            TryingSession::Starting(stub_id) => *stub_id == op_id,
            _ => false
        };
        if !start {
            return false;
        }

        self.trying = TryingSession::Running(session);
        true
    }

    fn on_query_finished(&mut self, owner: ChunkDownloader, op: &QueryContextOp, result: (LinkedList<DownloadSource<DeviceDesc>>, Timestamp)) -> SessionOp {
        let (mut sources, update_at) = result;

        if !self.querying.map(|stub_id| stub_id == op.op_id).unwrap_or(false) {
            info!("{} ignore queried sources for another query posted, op_id={}", owner, op.op_id);
            return SessionOp::None;
        }
        self.querying = None;

        let trying = self.trying.clone();
        match trying {
            TryingSession::None => {
                if update_at != self.update_at {
                    self.update_at = update_at;
                } 
                if sources.len() == 0 {
                    SessionOp::TrigerDrain(update_at)
                } else {
                    let op_id = self.gen_id.generate();
                    self.trying = TryingSession::Starting(op_id);
                    SessionOp::StartSession(StartSessionOp {
                        op_id, 
                        update_at, 
                        source: sources.pop_front().unwrap()
                    })
                }
            }, 
            TryingSession::Starting(_) => {
                info!("{} ignore queried sources for another session starting, op_id={}", owner, op.op_id);
                SessionOp::None
            }, 
            TryingSession::Running(session) => {
                if update_at != self.update_at {
                    self.update_at = update_at;
                    if sources.len() == 0 {
                        info!("{} cancel current session for context updated, op_id={}, session={}", owner, op.op_id, session);
                        session.cancel_by_error(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"));
                        self.tried.push_back(session);
                        self.trying = TryingSession::None;
    
                        let op_id = self.gen_id.generate();
                        self.querying = Some(op_id);
                        SessionOp::QueryContext(QueryContextOp {
                            op_id, 
                            filter: self.next_filter(), 
                            limit: 1
                        })
                    } else {
                        SessionOp::None
                    }
                } else {
                    unreachable!()
                }
            }
        }
    }
}



enum StateImpl {
    Loading, 
    Downloading(SingleStreamSession), 
    Finished
}

struct ChunkDowloaderImpl { 
    stack: WeakStack, 
    task: Box<dyn LeafDownloadTask>, 
    cache: ChunkCache, 
    state: RwLock<StateImpl>, 
}

#[derive(Clone)]
pub struct ChunkDownloader(Arc<ChunkDowloaderImpl>);

impl Drop for ChunkDowloaderImpl {
    fn drop(&mut self) {
        let session = {
            let state = &mut *self.state.write().unwrap();
            match state {
                StateImpl::Downloading(downloading) => downloading.trying.as_session().cloned(), 
                _ => None
            }
        };
       
        if let Some(session) = session {
            session.cancel_by_error(BuckyError::new(BuckyErrorCode::UserCanceled, "user canceled"));
        }
    }
}

pub struct WeakChunkDownloader(Weak<ChunkDowloaderImpl>);

impl WeakChunkDownloader {
    pub fn to_strong(&self) -> Option<ChunkDownloader> {
        Weak::upgrade(&self.0).map(|arc| ChunkDownloader(arc))
    }
}

impl ChunkDownloader {
    pub fn to_weak(&self) -> WeakChunkDownloader {
        WeakChunkDownloader(Arc::downgrade(&self.0))
    }
}

impl std::fmt::Display for ChunkDownloader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkDownloader{{chunk:{}}}", self.chunk())
    }
}


impl ChunkDownloader {
    pub fn new(
        stack: WeakStack, 
        cache: ChunkCache, 
        task: Box<dyn LeafDownloadTask>
    ) -> Self {
        let downloader = Self(Arc::new(ChunkDowloaderImpl {
            stack, 
            cache, 
            task, 
            state: RwLock::new(StateImpl::Loading), 
        }));

        {
            let downloader = downloader.clone();
            
            task::spawn(async move {
                info!("{} begin load cache", downloader);
                let finished = downloader.cache().wait_loaded().await;
                let update_at = task::block_on(downloader.owner().context().update_at());
                let op = {   
                    let state = &mut *downloader.0.state.write().unwrap();
                    if let StateImpl::Loading = state {
                        if finished {
                            *state = StateImpl::Finished;
                            info!("{} finished for cache exists", downloader);
                            None
                        } else {
                            info!("{} enter downloading", downloader);
                            let (downloading, op) = SingleStreamSession::new(update_at);
                            *state = StateImpl::Downloading(downloading);
                            Some(op)
                        }
                    } else {
                        unreachable!()
                    }
                };
               
                if let Some(op) = op {
                    {
                        let downloader = downloader.clone();
                        task::spawn(async move { downloader.sync_finished().await; });
                    }
                    downloader.query_context(op).await;
                }
                
            });
        }
        
        downloader
    }

    fn on_session_op(&self, op: SessionOp) {
        match op {
            SessionOp::None => {},
            SessionOp::QueryContext(op) => { 
                let downloader = self.clone(); 
                task::spawn(async move { downloader.query_context(op).await; }); 
            }, 
            SessionOp::TrigerDrain(update_at) => self.owner().context().on_drain(self.owner(), update_at), 
            SessionOp::StartSession(op) => { 
                let downloader = self.clone(); 
                task::spawn(async move { downloader.start_session(op).await; }); 
            }
        }
    }

    async fn query_context(&self, mut op: QueryContextOp) {
        op.filter.fill_values(self.chunk());
        let result = self.owner().context().sources_of(&op.filter, op.limit).await;
        info!("{} return sources from context, op_id={}, sources={:?}, update_at={}", self, op.op_id, result.0, result.1);
        let next_op = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                StateImpl::Downloading(downloading) => downloading.on_query_finished(self.clone(), &op, result),
                _ => SessionOp::None
            }
        };
        info!("{} will exec op after queried source, query_id={}, next_op={:?}", self, op.op_id, next_op);
        self.on_session_op(next_op)
    }

    async fn start_session(&self, op: StartSessionOp) {
        info!("{} will start session, op_id={}", self, op.op_id);

        let stack = Stack::from(&self.0.stack);
        let channel = stack.ndn().channel_manager().create_channel(&op.source.target).unwrap();   

        let mut source: DownloadSource<DeviceId> = op.source.into();
        source.codec_desc = match &source.codec_desc {
            ChunkCodecDesc::Unknown => ChunkCodecDesc::Stream(None, None, None).fill_values(self.chunk()), 
            ChunkCodecDesc::Stream(..) => source.codec_desc.fill_values(self.chunk()), 
            _ => unimplemented!()
        };

        let session = channel.download( 
            self.chunk().clone(), 
            source.clone(), 
            self.cache().stream().clone(), 
            Some(self.owner().context().referer().to_owned()), 
            self.owner().abs_group_path().clone()).or_else(|err| {
                Ok::<DownloadSession, ()>(DownloadSession::error(self.chunk().clone(), None, source, None, None, err))
            }).unwrap();

        let start = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                StateImpl::Downloading(downloading) => downloading.on_session_created(op.op_id, session.clone()), 
                _ => false
            }
        };

        if start {
            info!("{} will start session, op_id={}, session={}", self, op.op_id, session);
            session.start();
        } else {
            session.cancel_by_error(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"));
        }
        self.owner().context().on_new_session(self.owner(), &session, op.update_at);
    }

    async fn sync_finished(&self) {
        if self.cache().wait_exists(0..self.cache().chunk().len(), || self.owner().wait_user_canceled()).await.is_ok() {
            let state = &mut *self.0.state.write().unwrap();
            *state = StateImpl::Finished;
        }
    }

    async fn finished(&self) -> bool {
        if let StateImpl::Finished = &*self.0.state.read().unwrap() {
            true
        } else {
            false
        }
    }

    pub fn owner(&self) -> &dyn LeafDownloadTask {
        self.0.task.as_ref()
    }

    pub fn cache(&self) -> &ChunkCache {
        &self.0.cache
    }

    pub fn chunk(&self) -> &ChunkId {
        self.cache().chunk()
    }

    pub fn calc_speed(&self, when: Timestamp) -> u32 {
        match &*self.0.state.read().unwrap() {
            StateImpl::Downloading(downloading) => downloading.trying().map(|s| s.calc_speed(when)).unwrap_or_default(), 
            _ => 0
        }
    } 

    pub fn cur_speed(&self) -> u32 {
        match &*self.0.state.read().unwrap() {
            StateImpl::Downloading(downloading) => downloading.trying().map(|s| s.cur_speed()).unwrap_or_default(),
            _ => 0
        }
    }

    pub fn history_speed(&self) -> u32 {
        match &*self.0.state.read().unwrap() {
            StateImpl::Downloading(downloading) => downloading.trying().map(|s| s.history_speed()).unwrap_or_default(), 
            _ => 0
        }
    }

    pub fn on_drain(&self, _: u32) -> u32 {
        let update_at = task::block_on(self.owner().context().update_at());
        let (speed, op) = {
            let mut state = self.0.state.write().unwrap();
        
            match &mut *state{
                StateImpl::Downloading(downloading) => {
                    let speed = downloading.trying().map(|s| s.cur_speed()).unwrap_or_default();
                    let op = downloading.check_context(update_at);
                    (speed, op)
                }
                _ => (0, SessionOp::None)
            }
        };
        self.on_session_op(op);
        speed
    }
}
