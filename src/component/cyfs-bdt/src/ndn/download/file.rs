use std::{
    sync::{RwLock},
    ops::Range
};
use async_std::{
    sync::Arc, 
    task
};
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}, 
};
use super::super::{
    chunk::*, 
    channel::*
};
use super::{
    common::*, 
    chunk::ChunkTask, 
};

// TODO: 先实现最简单的顺序下载
struct DownloadingState {
    cur_index: usize, 
    cur_task: ChunkTask, 
    history_speed: HistorySpeed
}

enum TaskStateImpl {
    Pending, 
    Downloading(DownloadingState), 
    Error(BuckyErrorCode), 
    Finished
}

struct StateImpl {
    control_state: DownloadTaskControlState, 
    task_state: TaskStateImpl,
}

struct TaskImpl {
    stack: WeakStack, 
    file: File,
    chunk_list: ChunkListDesc, 
    ranges: Vec<(usize, Option<Range<u64>>)>, 
    context: SingleDownloadContext, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box<dyn ChunkWriterExt>>,
}

#[derive(Clone)]
pub struct FileTask(Arc<TaskImpl>);

impl std::fmt::Display for FileTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileTask::{{file:{}}}", self.file().desc().file_id())
    }
}

impl FileTask {
    pub fn new(
        stack: WeakStack,  
        file: File,  
        chunk_list: Option<ChunkListDesc>, 
        context: SingleDownloadContext, 
        writers: Vec<Box <dyn ChunkWriter>>, 
    ) -> Self {
        let chunk_list = chunk_list.unwrap_or(ChunkListDesc::from_file(&file).unwrap());

        let ranges = (0..chunk_list.chunks().len()).into_iter().map(|i| (i, None)).collect();

        Self(Arc::new(TaskImpl {
            stack, 
            file: file.clone(), 
            ranges, 
            chunk_list, 
            context, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Pending,
                control_state: DownloadTaskControlState::Normal,
            }), 
            writers: writers.into_iter().map(|w| ChunkWriterExtWrapper::new(w).clone_as_writer()).collect(),
        }))
    } 


    pub fn with_ranges(
        stack: WeakStack,  
        file: File,  
        chunk_list: Option<ChunkListDesc>, 
        ranges: Option<Vec<Range<u64>>>, 
        context: SingleDownloadContext, 
        writers: Vec<Box <dyn ChunkWriterExt>>, 
    ) -> Self {
        let chunk_list = chunk_list.unwrap_or(ChunkListDesc::from_file(&file).unwrap());
        let ranges = if ranges.is_none() {
            (0..chunk_list.chunks().len()).into_iter().map(|i| (i, None)).collect()
        } else {
            let ranges = ranges.unwrap();
            let mut dst_ranges = vec![];
            for range in ranges {
                for (index, range) in chunk_list.range_of(range) {
                    dst_ranges.push((index, Some(range)));
                }   
            } 
            dst_ranges
        };

        Self(Arc::new(TaskImpl {
            stack, 
            file: file.clone(), 
            chunk_list, 
            ranges, 
            context, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Pending, 
                control_state: DownloadTaskControlState::Normal,
            }), 
            writers,
        }))
    } 

    pub fn file(&self) -> &File {
        &self.0.file
    }

    pub fn chunk_list(&self) -> &ChunkListDesc {
        &self.0.chunk_list
    }

    pub fn ranges(&self) -> &Vec<(usize, Option<Range<u64>>)> {
        &self.0.ranges
    }

    pub fn context(&self) -> &SingleDownloadContext {
        &self.0.context
    }
}

#[async_trait::async_trait]
impl ChunkWriterExt for FileTask {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt> {
        Box::new(self.clone())
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        {
            let mut state = self.0.state.write().unwrap();
            state.task_state = TaskStateImpl::Error(err);
        }
        for writer in self.0.writers.iter() {
            let _ = writer.err(err).await;
        }
        Ok(())
    }


    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>, range: Option<Range<u64>>) -> BuckyResult<()> {
        let next_task = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.task_state {
                TaskStateImpl::Downloading(downloading) => {
                    let next_index = downloading.cur_index + 1;
                    if next_index == self.ranges().len() {
                        None
                    } else {
                        let (index, range) = self.ranges()[next_index].clone();
                        let chunk_task = ChunkTask::with_range(
                            self.0.stack.clone(), 
                            self.chunk_list().chunks()[index].clone(), 
                            range, 
                            self.context().clone(), 
                            vec![self.clone_as_writer()]
                        );
                        downloading.cur_index = next_index;
                        downloading.cur_task = chunk_task.clone();

                        Some(chunk_task)
                    }
                }, 
                _ => None
            }    
        };

        let finished = if let Some(task) = next_task {
            info!("{} create sub task {}", self, task);
            task.on_drain(self.history_speed());
            false
        } else {
            true
        };

        for writer in self.0.writers.iter() {
            let _ = writer.write(chunk, content.clone(), range.clone()).await?;
        }

        if finished {
            for writer in self.0.writers.iter() {
                let _ = writer.finish().await?;
            }
            let mut state = self.0.state.write().unwrap();
            info!("{} finished", self);
            state.task_state = TaskStateImpl::Finished;

        }

        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Ok(())
    }
}



impl DownloadTask for FileTask {
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
        self.0.state.read().unwrap().control_state.clone()
    }


    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                let cur_speed = downloading.cur_task.calc_speed(when);
                downloading.history_speed.update(Some(cur_speed), when);
                cur_speed
            }
            _ => 0,
        }
    }

    fn cur_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.cur_task.cur_speed(), 
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

    fn drain_score(&self) -> i64 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.cur_task.drain_score(), 
            _ => 0,
        }
    }

    fn on_drain(&self, expect_speed: u32) -> u32 {
        if let Some(sub_task) = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.task_state {
                TaskStateImpl::Pending => {
                    if self.ranges().len() > 0 {
                        let (index, range) = self.ranges()[0].clone();
                        let chunk_task = ChunkTask::with_range(
                            self.0.stack.clone(), 
                            self.chunk_list().chunks()[index].clone(),
                            range,  
                            self.context().clone(), 
                            vec![self.clone_as_writer()]
                        );
                        
                        let stack = Stack::from(&self.0.stack);
                        state.task_state = TaskStateImpl::Downloading(DownloadingState {
                            history_speed: HistorySpeed::new(0, stack.config().ndn.channel.history_speed.clone()), 
                            cur_index: 0, 
                            cur_task: chunk_task.clone()
                        });
                        info!("{} create sub task {}", self, chunk_task);
                        Some(chunk_task)
                    } else {
                        let file_task = self.clone();
                        task::spawn(async move {
                            let (chunk_len, chunk_data) = (0, vec![0u8; 0]);
                            let chunk_hash = hash_data(&chunk_data[..]);
                            let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
                            let chunk_data = Arc::new(chunk_data);
                            for writer in file_task.0.writers.iter() {
                                let _ = writer.write(&chunkid, chunk_data.clone(), None).await;
                                let _ = writer.finish().await;
                            }
                            let mut state = file_task.0.state.write().unwrap();
                            info!("{} finished", file_task);
                            state.task_state = TaskStateImpl::Finished;
                        });
                        None
                    }
                },
                TaskStateImpl::Downloading(downloading) => Some(downloading.cur_task.clone()), 
                _ => None
            } 
        } {
            sub_task.on_drain(expect_speed)
        } else {
            0
        }
    }
}