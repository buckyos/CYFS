use std::{
    sync::{RwLock, Mutex},
    ops::Range
};
use async_std::{
    sync::Arc, 
    task
};
use cyfs_base::*;
use crate::{
    stack::{WeakStack}, 
};
use super::super::{
    chunk::*, 
    scheduler::*, 
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
    schedule_state: TaskStateImpl,
}

struct TaskImpl {
    stack: WeakStack, 
    file: File,
    chunk_list: ChunkListDesc, 
    ranges: Vec<(usize, Option<Range<u64>>)>, 
    config: Mutex<Arc<ChunkDownloadConfig>>, 
    resource: ResourceManager, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box<dyn ChunkWriterExt>>,
}

#[derive(Clone)]
pub struct FileTask(Arc<TaskImpl>);

impl std::fmt::Display for FileTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileTask::{{file:{}, config:{:?}}}", self.file().desc().file_id(), self.config())
    }
}

impl FileTask {
    pub fn new(
        stack: WeakStack,  
        file: File,  
        chunk_list: Option<ChunkListDesc>, 
        config: Arc<ChunkDownloadConfig>, 
        writers: Vec<Box <dyn ChunkWriter>>, 
        owner: ResourceManager,
    ) -> Self {
        let chunk_list = chunk_list.unwrap_or(ChunkListDesc::from_file(&file).unwrap());

        Self(Arc::new(TaskImpl {
            stack, 
            file: file.clone(), 
            ranges: (0..chunk_list.chunks().len()).into_iter().map(|i| (i, None)).collect(),  
            chunk_list, 
            config : Mutex::new(Arc::new(config.as_ref().clone())),
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0),
            }), 
            writers: writers.into_iter().map(|w| ChunkWriterExtWrapper::new(w).clone_as_writer()).collect(),
        }))
    } 


    pub fn with_ranges(
        stack: WeakStack,  
        file: File,  
        chunk_list: Option<ChunkListDesc>, 
        ranges: Option<Vec<Range<u64>>>, 
        config: Arc<ChunkDownloadConfig>, 
        writers: Vec<Box <dyn ChunkWriterExt>>, 
        owner: ResourceManager,
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
            config: Mutex::new(Arc::new(config.as_ref().clone())),
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0),
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

    pub fn config(&self) -> Arc<ChunkDownloadConfig> {
        self.0.config.lock().unwrap().clone()
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
                            self.config().clone(), 
                            vec![self.clone_as_writer()], 
                            self.resource().clone());
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

impl TaskSchedule for FileTask {
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
                self.config().clone(), 
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
                state.schedule_state = TaskStateImpl::Finished;
                state.control_state = TaskControlState::Finished(file_task.resource().avg_usage().downstream_bandwidth());
            });
            TaskState::Running(0)
        }
    }
}

impl DownloadTaskControl for FileTask {
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

impl DownloadTask for FileTask {
    fn clone_as_download_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }
}
