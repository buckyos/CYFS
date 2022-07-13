use std::{
    sync::RwLock, 
    ops::Range
};
use async_std::{
    sync::Arc, 
    task
};
use cyfs_base::*;
use crate::{
    stack::{WeakStack, Stack}
};
use super::super::{
    chunk::*, 
    scheduler::*, 
};


enum TaskStateImpl {
    Pending, 
    Downloading(ChunkDownloader),
    Canceled(BuckyErrorCode),  
    Writting,  
    Finished,
}

struct StateImpl {
    control_state: TaskControlState, 
    schedule_state: TaskStateImpl,
    chunk_statistic_task: DynamicStatisticTask,
}

struct ChunkTaskImpl {
    stack: WeakStack, 
    chunk: ChunkId, 
    range: Option<Range<u64>>, 
    config: Arc<ChunkDownloadConfig>, 
    resource: ResourceManager, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box <dyn ChunkWriterExt>>,
    statistic_task_cb: Option<StatisticTaskPtr>,
}

#[derive(Clone)]
pub struct ChunkTask(Arc<ChunkTaskImpl>);


impl std::fmt::Display for ChunkTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkTask{{chunk:{}, range:{:?}, config:{:?}}}", self.chunk(), self.range(), self.config())
    }
}

impl ChunkTask {
    pub fn new(
        stack: WeakStack, 
        chunk: ChunkId, 
        config: Arc<ChunkDownloadConfig>, 
        writers: Vec<Box <dyn ChunkWriter>>, 
        owner: ResourceManager,
        statistic_task_cb: Option<StatisticTaskPtr>) -> Self {
        Self(Arc::new(ChunkTaskImpl {
            stack, 
            chunk, 
            range: None, 
            config, 
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0),
                chunk_statistic_task: DynamicStatisticTask::default(),
            }), 
            writers: writers.into_iter().map(|w| ChunkWriterExtWrapper::new(w).clone_as_writer()).collect(),
            statistic_task_cb: statistic_task_cb
        }))
    } 

    pub fn with_range(
        stack: WeakStack, 
        chunk: ChunkId, 
        range: Option<Range<u64>>, 
        config: Arc<ChunkDownloadConfig>, 
        writers: Vec<Box <dyn ChunkWriterExt>>, 
        owner: ResourceManager,
        statistic_task_cb: Option<StatisticTaskPtr>) -> Self {
        Self(Arc::new(ChunkTaskImpl {
            stack, 
            chunk, 
            range, 
            config, 
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0),
                chunk_statistic_task: DynamicStatisticTask::default(),
            }), 
            writers,
            statistic_task_cb: statistic_task_cb
        }))
    } 


    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn range(&self) -> Option<Range<u64>> {
        self.0.range.clone()
    }

    pub fn config(&self) -> &Arc<ChunkDownloadConfig> {
        &self.0.config
    }

    async fn sync_chunk_state(&self) {
        loop {
            let downloader = {
                match &self.0.state.read().unwrap().schedule_state {
                    TaskStateImpl::Downloading(downloader) => downloader.clone(), 
                    _ => unreachable!()
                }
            };

            match downloader.wait_finish().await {
                TaskState::Finished => {
                    match downloader.reader().unwrap().get(self.chunk()).await {
                        Ok(content) => {
                            self.0.state.write().unwrap().schedule_state = TaskStateImpl::Writting;
                            for writer in &self.0.writers {
                                let _ = writer.write(self.chunk(), content.clone(), self.range()).await;
                                let _ = writer.finish().await;
                            }
                            let mut state = self.0.state.write().unwrap();
                            info!("{} finished", self);
                            state.schedule_state = TaskStateImpl::Finished;
                            state.control_state = TaskControlState::Finished;
                            break; 
                        }, 
                        Err(_err) => {
                            let stack = Stack::from(&self.0.stack);
                            let downloader = stack.ndn().chunk_manager().start_download(
                                self.chunk().clone(), 
                                self.config().clone(), 
                                self.resource().clone(),
                                self.as_statistic()).await.unwrap();
                            info!("{} reset downloader for read chunk failed", self);
                            let mut state = self.0.state.write().unwrap();
                            state.schedule_state = TaskStateImpl::Downloading(downloader.clone());
                        }
                    }
                    
                }, 
                TaskState::Canceled(err) => {
                    error!("{} canceled", self);
                    self.0.state.write().unwrap().schedule_state = TaskStateImpl::Canceled(err);
                    
                    self.0.state.write().unwrap().schedule_state = TaskStateImpl::Writting;
                    for writer in &self.0.writers {
                        let _ = writer.err(err).await;
                    }

                    break; 
                }, 
                TaskState::Redirect(redirect_node, redirect_referer) => {
                    info!("{} redirect {} referer {}", self, redirect_node, redirect_referer);
                    // i need a new task for redirection task
                    let device_id = DeviceId::try_from(redirect_node.clone());
                    if device_id.is_err() {
                        error!("failed try_from deviceid, {}", redirect_node);
                        for writer in &self.0.writers {
                            let _ = writer.err(BuckyErrorCode::InvalidParam).await;
                        }
                        break;
                    }

                    let device_id = device_id.unwrap();
                    let mut config = ChunkDownloadConfig::force_stream(device_id);
                    config.referer = Some(redirect_referer);

                    let stack = Stack::from(&self.0.stack);
                    let downloader = stack.ndn().chunk_manager().start_download(
                        self.chunk().clone(), 
                        Arc::new(config), 
                        self.resource().clone(),
			self.as_statistic()).await.unwrap();

                    info!("{} create a new downloader for redirection task", self);
                    let mut state = self.0.state.write().unwrap();
                    state.schedule_state = TaskStateImpl::Downloading(downloader.clone());
                },
                TaskState::WaitRedirect => {
                    let stack = Stack::from(&self.0.stack);

                    async_std::future::timeout(stack.config().ndn.channel.wait_redirect_timeout, async_std::future::pending::<()>());

                    let downloader = stack.ndn().chunk_manager().start_download(
                        self.chunk().clone(), 
                        self.config().clone(), 
                        self.resource().clone(),
			self.as_statistic()).await.unwrap();
                    info!("{} reset downloader for read chunk failed", self);
                    let mut state = self.0.state.write().unwrap();
                    state.schedule_state = TaskStateImpl::Downloading(downloader.clone());
            },
                _ => unimplemented!()
            }
        }
    }

    pub fn as_statistic(&self) -> Arc<dyn StatisticTask> {
        Arc::from(self.clone())
    }

}

impl TaskSchedule for ChunkTask {
    fn schedule_state(&self) -> TaskState {
        match &self.0.state.read().unwrap().schedule_state {
            TaskStateImpl::Pending => TaskState::Pending, 
            TaskStateImpl::Downloading(downloader) => downloader.schedule_state(), 
            TaskStateImpl::Canceled(err) => TaskState::Canceled(*err), 
            TaskStateImpl::Writting => TaskState::Running(0), 
            // TaskStateImpl::Redirect(redirect_node, referer) => TaskState::Redirect(redirect_node.clone(), referer.clone()),
            TaskStateImpl::Finished => TaskState::Finished
        }
    }

    fn resource(&self) -> &ResourceManager {
        &self.0.resource
    }

    fn statistic_task(&self) -> Option<DynamicStatisticTask> {
        Some(self.0.state.read().unwrap().chunk_statistic_task.clone())
    }

    //保证不会重复调用
    fn start(&self) -> TaskState {
        info!("{} started", self);
        let stack = Stack::from(&self.0.stack);
        let task = self.clone();
        task::spawn(async move {
            let downloader = stack.ndn().chunk_manager().start_download(
                task.chunk().clone(), 
                task.config().clone(), 
                task.resource().clone(),
                task.as_statistic()).await.unwrap();
        
            {
                let mut state = task.0.state.write().unwrap();
                match &state.schedule_state {
                    TaskStateImpl::Pending => {
                        state.schedule_state = TaskStateImpl::Downloading(downloader.clone())
                    }, 
                    _ => unreachable!()
                }
            }
            task.sync_chunk_state().await;
        });
        
        TaskState::Running(0)
    }
}

impl DownloadTaskControl for ChunkTask {
    fn control_state(&self) -> TaskControlState {
        self.0.state.read().unwrap().control_state.clone()
    }

    fn pause(&self) -> BuckyResult<TaskControlState> {
        unimplemented!()
    }

    fn resume(&self) -> BuckyResult<TaskControlState> {
        unimplemented!()
    }

    fn cancel(&self) -> BuckyResult<TaskControlState> {
        unimplemented!()
    }
}

impl DownloadTask for ChunkTask {
    fn clone_as_download_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }
}

impl StatisticTask for ChunkTask {
    fn reset(&self) {
        self.0.state.write().unwrap().chunk_statistic_task.reset();
    }

    fn stat(&self) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        if let Some(cb) = &self.0.statistic_task_cb {
            cb.stat()
        } else {
            Ok(PerfData::default().clone_as_perfdata())
        }
    }

    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        let _ = self.0.state.write().unwrap().chunk_statistic_task.on_stat(size);

        if let Some(cb) = &self.0.statistic_task_cb {
            cb.on_stat(size)
        } else {
            Ok(PerfData::default().clone_as_perfdata())
        }
    }

}