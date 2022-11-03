use std::{
    sync::{RwLock},
};
use async_std::{
    sync::Arc, 
    pin::Pin, 
    task::{Context, Poll},
    task
};
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}, 
};
use super::super::{
    chunk::*, 
    types::*
};
use super::{
    common::*, 
};

struct DownloadingState {
    cur_cache: ChunkCache, 
    history_speed: HistorySpeed
}

enum TaskStateImpl {
    Pending, 
    Downloading(DownloadingState), 
    Finished, 
    Error(BuckyErrorCode)
}

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

struct StateImpl {
    control_state: ControlStateImpl, 
    task_state: TaskStateImpl,
}

struct TaskImpl {
    stack: WeakStack, 
    name: String, 
    chunk_list: ChunkListDesc, 
    context: SingleDownloadContext, 
    state: RwLock<StateImpl>,  
}

#[derive(Clone)]
pub struct ChunkListTask(Arc<TaskImpl>);

impl std::fmt::Display for ChunkListTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkListTask:{}", self.0.name)
    }
}


impl ChunkListTask {
    pub fn new(
        stack: WeakStack,  
        name: String, 
        chunk_list: ChunkListDesc, 
        context: SingleDownloadContext, 
    ) -> Self {
        let task = Self(Arc::new(TaskImpl {
            stack, 
            name, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Pending, 
                control_state: ControlStateImpl::Normal(StateWaiter::new()),
            }), 
            chunk_list, 
            context, 
        }));

        {
            let task = task.clone();
            task::spawn(async move {
                task.begin().await;
            });
        }
       
        task
    } 

    async fn begin(&self) {
        let stack = Stack::from(&self.0.stack);

        for chunk in self.chunk_list().chunks() {
            let cache = stack.ndn().chunk_manager().create_cache(chunk);
            cache.downloader().context().add_context(self.context().clone());

            {
                let mut state = self.0.state.write().unwrap();
                match &state.task_state {
                    TaskStateImpl::Pending => {
                        state.task_state = TaskStateImpl::Downloading(DownloadingState {
                            cur_cache: cache.clone(), 
                            history_speed: HistorySpeed::new(0, stack.config().ndn.channel.history_speed.clone())
                        })
                    }, 
                    TaskStateImpl::Downloading(_) => {
                        state.task_state = TaskStateImpl::Downloading(DownloadingState {
                            cur_cache: cache.clone(), 
                            history_speed: HistorySpeed::new(0, stack.config().ndn.channel.history_speed.clone())
                        })
                    },
                    _ => {
    
                    }
                }
            }
           
            if cache.wait_exists(0..chunk.len(), || self.wait_user_canceled()).await.is_err() {
                return;
            }
        }

        let mut state = self.0.state.write().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(_) => {
                state.task_state = TaskStateImpl::Finished
            },
            _ => {}
        }
    }


    pub fn chunk_list(&self) -> &ChunkListDesc {
        &self.0.chunk_list
    }

    pub fn context(&self) -> &SingleDownloadContext {
        &self.0.context
    }
}


#[async_trait::async_trait]
impl DownloadTask for ChunkListTask {
    fn context(&self) -> &SingleDownloadContext {
        &self.0.context
    }
    
    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Pending => DownloadTaskState::Downloading(0 ,0.0), 
            TaskStateImpl::Downloading(_) => DownloadTaskState::Downloading(0 ,0.0), 
            TaskStateImpl::Finished => DownloadTaskState::Finished, 
            TaskStateImpl::Error(err) => DownloadTaskState::Error(*err), 
        }
    }

    fn control_state(&self) -> DownloadTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => DownloadTaskControlState::Normal, 
            ControlStateImpl::Canceled => DownloadTaskControlState::Canceled
        }
    }


    fn calc_speed(&self, when: Timestamp) -> u32 {
        let state = self.0.state.write().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                let cur_speed = downloading.cur_cache.downloader().calc_speed();
                downloading.history_speed.update(Some(cur_speed), when);
                cur_speed
            }, 
            _ => 0
        }
    }

    fn cur_speed(&self) -> u32 {
        if let Some(cache) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => Some(downloading.cur_cache.clone()), 
                _ => None
            }
        } {
            cache.downloader().cur_speed()
        } else {
            0
        }
    }

    fn history_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => downloading.history_speed.average(), 
                _ => 0
            }
    }

    fn drain_score(&self) -> i64 {
        if let Some(cache) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => Some(downloading.cur_cache.clone()), 
                _ => None
            }
        } {
            cache.downloader().drain_score()
        } else {
            0
        }
    }

    fn on_drain(&self, expect_speed: u32) -> u32 {
        if let Some(cache) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => Some(downloading.cur_cache.clone()), 
                _ => None
            }
        } {
            cache.downloader().on_drain(expect_speed)
        } else {
            0
        }
    }


    fn cancel(&self) -> BuckyResult<DownloadTaskControlState> {
        let (cache, waiters) = {
            let mut state = self.0.state.write().unwrap();
            let waiters = match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => {
                    let waiters = Some(waiters.transfer());
                    state.control_state = ControlStateImpl::Canceled;
                    waiters
                }, 
                _ => None
            };

            let cache = match &state.task_state {
                TaskStateImpl::Downloading(downloading) => {
                    let cache = Some(downloading.cur_cache.clone());
                    state.task_state = TaskStateImpl::Error(BuckyErrorCode::UserCanceled);
                    cache
                }, 
                _ => None
            };

            (cache, waiters)
        };

        if let Some(waiters) = waiters {
            waiters.wake();
        }

        if let Some(cache) = cache {
            let strong_stack = Stack::from(&self.0.stack);
            cache.downloader().context().remove_context(self.context(), self.state());
        }
        
        Ok(DownloadTaskControlState::Canceled)
    }
    

    async fn wait_user_canceled(&self) -> BuckyError {
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => Some(waiters.new_waiter()), 
                _ => None
            }
        };
        
        
        if let Some(waiter) = waiter {
            let _ = StateWaiter::wait(waiter, || self.control_state()).await;
        } 

        BuckyError::new(BuckyErrorCode::UserCanceled, "")
    }
}



pub struct ChunkListTaskReader {
    offset: u64,
    task: ChunkListTask
}

impl ChunkListTaskReader {
    pub fn new(task: ChunkListTask, offset: usize) -> Self {
        Self {
            offset: 0, 
            task
        }
    }
}


impl async_std::io::Read for ChunkListTaskReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let pined = self.get_mut();
        let ranges = pined.task.chunk_list().range_of(pined.offset..pined.offset + buffer.len() as u64);
        if ranges.is_empty() {
            return Poll::Ready(Ok(0));
        }
        if let DownloadTaskState::Error(err) = pined.task.state() {
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, BuckyError::new(err, ""))));
        } 
        let (index, range) = ranges[0];
        let chunk = pined.task.chunk_list().chunks()[index];
        let stack = Stack::from(&pined.task.0.stack);
        let cache = stack.ndn().chunk_manager().create_cache(&chunk);
        let mut reader = DownloadTaskReader::new(cache, pined.task.clone_as_task());
        use std::{io::{Seek, SeekFrom}};
        match reader.seek(SeekFrom::Start(range.start)) {
            Ok(_) => {
                let result = async_std::io::Read::poll_read(Pin::new(&mut reader), cx, &mut buffer[0..(range.end - range.start) as usize]);
                if let Poll::Ready(result) = &result {
                    if let Ok(len) = result {
                        pined.offset += *len as u64;
                    }
                }
                result
            },
            Err(err) => Poll::Ready(Err(err))
        }
    }
}
