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
    stack::{WeakStack}
};
use super::super::{
    chunk::*, 
    scheduler::*, 
    download::*
};
use super::{
    chunk::ChunkTask,
};

// TODO: 先实现最简单的顺序下载
struct DownloadingState {
    cur_index: usize, 
    cur_task: ChunkTask
}

enum TaskStateImpl {
    Pending, 
    Downloading(DownloadingState), 
    Finished
}

struct StateImpl {
    control_state: TaskControlState, 
    schedule_state: TaskStateImpl
}


struct TaskImpl {
    stack: WeakStack, 
    name: String, 
    chunk_list: ChunkListDesc, 
    ranges: Vec<(usize, Option<Range<u64>>)>, 
    context: SingleDownloadContext, 
    resource: ResourceManager, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box<dyn ChunkWriterExt>>,
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
        writers: Vec<Box <dyn ChunkWriter>>, 
        owner: ResourceManager
    ) -> Self {

        Self(Arc::new(TaskImpl {
            stack, 
            name, 
            ranges: (0..chunk_list.chunks().len()).into_iter().map(|i| (i, None)).collect(), 
            chunk_list, 
            context, 
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0)
            }), 
            writers: writers.into_iter().map(|w| ChunkWriterExtWrapper::new(w).clone_as_writer()).collect(),
        }))
    } 

    pub fn with_ranges(
        stack: WeakStack,
        name: String, 
        chunk_list: ChunkListDesc, 
        ranges: Option<Vec<Range<u64>>>, 
        context: SingleDownloadContext, 
        writers: Vec<Box <dyn ChunkWriterExt>>, 
        owner: ResourceManager,
    ) -> Self {
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
            name, 
            chunk_list, 
            ranges, 
            context, 
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0)
            }), 
            writers
        }))
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
impl ChunkWriterExt for ChunkListTask {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt> {
        Box::new(self.clone())
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        {
            let mut state = self.0.state.write().unwrap();
            state.control_state = TaskControlState::Err(err);
        }
        for writer in self.0.writers.iter() {
            let _ = writer.err(err).await;
        }
        Ok(())
    }

    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>, range: Option<Range<u64>>) -> BuckyResult<()> {
        let (cur_task, next_task) = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.schedule_state {
                TaskStateImpl::Downloading(downloading) => {
                    let next_index = downloading.cur_index + 1;
                    let cur_task = Some(downloading.cur_task.clone());
                    if next_index == self.ranges().len() {
                        (cur_task, None)
                    } else {
                        let (index, range) = self.ranges()[next_index].clone();
                        let chunk_task = ChunkTask::with_range(
                            self.0.stack.clone(), 
                            self.chunk_list().chunks()[index].clone(), 
                            range, 
                            self.context().clone(), 
                            vec![self.clone_as_writer()], 
                            self.resource().clone(),
                        );
                        downloading.cur_index = next_index;
                        downloading.cur_task = chunk_task.clone();
                        (cur_task, Some(chunk_task))
                    }
                }, 
                _ => (None, None)
            }    
        };

        if let Some(cur_task) = cur_task {
            let _ = self.resource().remove_child(cur_task.resource());
            info!("{} remove sub task {}", self, cur_task);
        }
        
        let finished = if let Some(task) = next_task {
            info!("{} create sub task {}", self, task);
            task.start();
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
            state.schedule_state = TaskStateImpl::Finished;
            state.control_state = TaskControlState::Finished(self.resource().avg_usage().downstream_bandwidth());
        }

        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Ok(())
    }
}

impl TaskSchedule for ChunkListTask {
    fn schedule_state(&self) -> TaskState {
        match &self.0.state.read().unwrap().schedule_state {
            TaskStateImpl::Pending => TaskState::Pending, 
            TaskStateImpl::Downloading(_) => TaskState::Running(0), 
            TaskStateImpl::Finished => TaskState::Finished
        }
    }

    fn resource(&self) -> &ResourceManager {
        &self.0.resource
    }

    //保证不会重复调用
    fn start(&self) -> TaskState {
        info!("{} started", self);
        if self.ranges().len() > 0 {
            let (index, range) = self.ranges()[0].clone();
            let chunk_task = ChunkTask::with_range(
                self.0.stack.clone(), 
                self.chunk_list().chunks()[index].clone(),
                range,  
                self.context().clone(), 
                vec![self.clone_as_writer()], 
                self.resource().clone());
            let mut state = self.0.state.write().unwrap();
            match &state.schedule_state {
                TaskStateImpl::Pending => {
                    state.schedule_state = TaskStateImpl::Downloading(DownloadingState {
                        cur_index: 0, 
                        cur_task: chunk_task.clone()
                    });
                }, 
                _ => unreachable!()
            }
            info!("{} create sub task {}", self, chunk_task);
            chunk_task.start();
            TaskState::Running(0)
        } else {
            let file_task = self.clone();
            task::spawn(async move {
                for writer in file_task.0.writers.iter() {
                    let _ = writer.finish().await;
                }
                let mut state = file_task.0.state.write().unwrap();
                info!("{} finished", file_task);
                state.schedule_state = TaskStateImpl::Finished;
                state.control_state = TaskControlState::Finished(0);
            });
            TaskState::Running(0)
        }
    }
}

impl DownloadTaskControl for ChunkListTask {
    fn control_state(&self) -> TaskControlState {
        self.0.state.read().unwrap().control_state.clone()
    }

    fn pause(&self) -> BuckyResult<TaskControlState> {
        Ok(self.control_state())
        //unimplemented!()
    }

    fn resume(&self) -> BuckyResult<TaskControlState> {
        Ok(self.control_state())
        //unimplemented!()
    }

    fn cancel(&self) -> BuckyResult<TaskControlState> {
        Ok(self.control_state())
        //unimplemented!()
    }
}

impl DownloadTask for ChunkListTask {
    fn clone_as_download_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }
}