use std::{
    sync::{RwLock},
    io::SeekFrom, 
    ops::Range
};
use async_std::{
    sync::Arc, 
    pin::Pin, 
    task::{Context, Poll}, 
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
    downloaded: u64,  
    cur_speed: ProgressCounter,  
    cur_chunk: (ChunkDownloader, usize), 
    history_speed: HistorySpeed,
}

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

enum TaskStateImpl {
    Pending, 
    Downloading(DownloadingState), 
    Error(BuckyError), 
    Finished(u64)
}

struct StateImpl {
    abs_path: Option<String>, 
    control_state: ControlStateImpl, 
    task_state: TaskStateImpl,
}

struct TaskImpl {
    stack: WeakStack, 
    name: String, 
    chunk_list: ChunkListDesc,
    context: Box<dyn DownloadContext>,
    state: RwLock<StateImpl>,  
}

#[derive(Clone)]
pub struct ChunkListTask(Arc<TaskImpl>);

impl std::fmt::Display for ChunkListTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkListTask::{{name:{}}}", self.name())
    }
}

impl ChunkListTask {
    pub fn new(
        stack: WeakStack,  
        name: String,
        chunk_list: ChunkListDesc, 
        context: Box<dyn DownloadContext>, 
    ) -> Self {
        Self(Arc::new(TaskImpl {
            stack, 
            name,
            context, 
            state: RwLock::new(StateImpl {
                abs_path: None, 
                task_state: if chunk_list.total_len() > 0 {
                    TaskStateImpl::Pending
                } else {
                    TaskStateImpl::Finished(0)
                },
                control_state: ControlStateImpl::Normal(StateWaiter::new()),
            }),
            chunk_list, 
        }))
    } 

    pub fn name(&self) -> &str {
        self.0.name.as_str()
    }

    pub fn chunk_list(&self) -> &ChunkListDesc {
        &self.0.chunk_list
    }

    fn create_cache(&self, index: usize) -> BuckyResult<ChunkCache> {
        let stack = Stack::from(&self.0.stack);
        let chunk = &self.chunk_list().chunks()[index];

        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Pending => {
                debug!("{} create cache from pending, index={}, chunk={}", self, index, chunk);
                let downloader = stack.ndn().chunk_manager().create_downloader(chunk, self.clone_as_leaf_task());
                state.task_state = TaskStateImpl::Downloading(DownloadingState { 
                    downloaded: 0, 
                    cur_speed: ProgressCounter::new(0), 
                    cur_chunk: (downloader.clone(), index), 
                    history_speed: HistorySpeed::new(0, stack.config().ndn.channel.history_speed.clone()), 
                });
                Ok(downloader.cache().clone())
            }, 
            TaskStateImpl::Downloading(downloading) => {
                let (downloader, cur_index) = &downloading.cur_chunk;
                if *cur_index != index {
                    debug!("{} create new cache, old_index={}, old_chunk={}, index={}, chunk={}", self, *cur_index, downloader.cache().chunk(), index, chunk);
                    downloading.downloaded += downloader.cache().stream().len() as u64;
                    downloading.cur_chunk = (stack.ndn().chunk_manager().create_downloader(chunk, self.clone_as_leaf_task()), index);
                }
                Ok(downloading.cur_chunk.0.cache().clone())
            },
            TaskStateImpl::Finished(_) => unreachable!(), 
            TaskStateImpl::Error(err) => Err(err.clone())
        }
    }
}

#[async_trait::async_trait]
impl LeafDownloadTask for ChunkListTask {
    fn clone_as_leaf_task(&self) -> Box<dyn LeafDownloadTask> {
        Box::new(self.clone())
    }

    fn abs_group_path(&self) -> Option<String> {
        self.0.state.read().unwrap().abs_path.clone()
    }

    fn context(&self) -> &dyn DownloadContext {
        self.0.context.as_ref()
    }

    fn finish(&self) {
        let mut state = self.0.state.write().unwrap();

        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                downloading.downloaded += downloading.cur_chunk.0.cache().stream().len() as u64;
                state.task_state = TaskStateImpl::Finished(downloading.downloaded);
            }, 
            _ => {}
        };
    }
}

impl NdnTask for ChunkListTask {
    fn clone_as_task(&self) -> Box<dyn NdnTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> NdnTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Pending => NdnTaskState::Running, 
            TaskStateImpl::Downloading(_) => NdnTaskState::Running, 
            TaskStateImpl::Finished(_) => NdnTaskState::Finished, 
            TaskStateImpl::Error(err) => NdnTaskState::Error(err.clone()),
        }
    }

    fn control_state(&self) -> NdnTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => NdnTaskControlState::Normal, 
            ControlStateImpl::Canceled => NdnTaskControlState::Canceled
        }
    }

    fn cur_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.history_speed.latest(), 
            _ => 0,
        }
    }

    fn history_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.history_speed.average(), 
            _ => 0,
        }
    }

    fn transfered(&self) -> u64 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.downloaded + downloading.cur_chunk.0.cache().stream().len() as u64, 
            TaskStateImpl::Finished(downloaded) => *downloaded, 
            _ => 0,
        }

    }

    fn cancel_by_error(&self, err: BuckyError) -> BuckyResult<NdnTaskControlState> {
        let waiters = {
            let mut state = self.0.state.write().unwrap();
            let waiters = match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => {
                    let waiters = Some(waiters.transfer());
                    state.control_state = ControlStateImpl::Canceled;
                    waiters
                }, 
                _ => None
            };

            match &state.task_state {
                TaskStateImpl::Downloading(_) => {
                    state.task_state = TaskStateImpl::Error(err);
                }, 
                _ => {}
            };

            waiters
        };

        if let Some(waiters) = waiters {
            waiters.wake();
        }

        Ok(NdnTaskControlState::Canceled)
    }
}

#[async_trait::async_trait]
impl DownloadTask for ChunkListTask {
    fn clone_as_download_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn on_post_add_to_root(&self, abs_path: String) {
        self.0.state.write().unwrap().abs_path = Some(abs_path);
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                let downloaded = downloading.downloaded + downloading.cur_chunk.0.cache().stream().len() as u64;
                let cur_speed = downloading.cur_speed.update(downloaded, when);
                debug!("{} calc_speed update cur_speed {}", self, cur_speed);
                downloading.history_speed.update(Some(cur_speed), when);
                cur_speed
            }
            _ => 0,
        }
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
    fn new(task: ChunkListTask) -> Self {
        Self {
            offset: 0, 
            task
        }
    }

    pub fn task(&self) -> &dyn LeafDownloadTask {
        &self.task
    }
}

impl Drop for ChunkListTaskReader {
    fn drop(&mut self) {
        if self.offset == self.task.chunk_list().total_len() {
            self.task.finish();
        } else {
            let _ = self.task.cancel();
        }
    }
}

impl std::io::Seek for ChunkListTaskReader {
    fn seek(
        self: &mut Self,
        pos: SeekFrom,
    ) -> std::io::Result<u64> {
        let len = self.task.chunk_list().total_len();
        let new_offset = match pos {
            SeekFrom::Start(offset) => len.min(offset), 
            SeekFrom::Current(offset) => {
                let offset = (self.offset as i64) + offset;
                let offset = offset.max(0) as u64;
                len.min(offset)
            },
            SeekFrom::End(offset) => {
                let offset = (len as i64) + offset;
                let offset = offset.max(0) as u64;
                len.min(offset)
            }
        };
        if new_offset < self.offset {
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "single directed stream"))
        } else {
            self.offset = new_offset;

            Ok(new_offset)
        }
    }
}


impl DownloadTaskSplitRead for ChunkListTaskReader {
    fn poll_split_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<Option<(ChunkCache, Range<usize>)>>> {
        let pined = self.get_mut();
        debug!("{} poll split read, buffer={}, offset={}", pined.task(), buffer.len(), pined.offset);
        let ranges = pined.task.chunk_list().range_of(pined.offset..pined.offset + buffer.len() as u64);
        if ranges.is_empty() {
            debug!("{} poll split read break, buffer={}, offset={}", pined.task(), buffer.len(), pined.offset);
            return Poll::Ready(Ok(None));
        }
        if let NdnTaskState::Error(err) = pined.task.state() {
            debug!("{} poll split read break, buffer={}, offset={}", pined.task(), buffer.len(), pined.offset);
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, BuckyError::new(err, ""))));
        } 
        let (index, range) = ranges[0].clone();
        debug!("{} poll split on chunk index, buffer={}, offset={}, index={}", pined.task(), buffer.len(), pined.offset, index);
        let result = match pined.task.create_cache(index) {
            Ok(cache) => {
                let mut reader = DownloadTaskReader::new(cache, pined.task.clone_as_leaf_task());
                use std::{io::{Seek}};
                match reader.seek(SeekFrom::Start(range.start)) {
                    Ok(_) => {
                        let result = DownloadTaskSplitRead::poll_split_read(Pin::new(&mut reader), cx, &mut buffer[0..(range.end - range.start) as usize]);
                        if let Poll::Ready(result) = &result {
                            if let Some((_, r)) = result.as_ref().ok().and_then(|r| r.as_ref()) {
                                let old_offset = pined.offset;
                                pined.offset += (r.end - r.start) as u64;
                                debug!("{} poll split offset changed, buffer={}, offset={}, new_offset={}", pined.task(), buffer.len(), old_offset, pined.offset);
                            }
                        }
                        result
                    },
                    Err(err) => {
                        return Poll::Ready(Err(err));
                    }
                }
            }
            Err(err) => {
                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, err)));
            }
        };
        result
    }
}


impl async_std::io::Read for ChunkListTaskReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        self.poll_split_read(cx, buffer).map(|result| result.map(|r| if let Some((_, r)) = r {
            r.end - r.start
        } else {
            0
        }))
    }
}


impl ChunkListTask {
    pub fn reader(
        stack: WeakStack,  
        name: String,
        chunk_list: ChunkListDesc, 
        context: Box<dyn DownloadContext>
    ) -> (Self, ChunkListTaskReader) {
        let task = Self::new(stack, name, chunk_list, context);
        let reader = ChunkListTaskReader::new(task.clone());
        
        (task, reader)
    }
}