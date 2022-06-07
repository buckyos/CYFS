use std::{
    sync::RwLock,
    collections::BTreeMap, 
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
    statistic::*
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
    chunk_statistic_task: BTreeMap<ChunkId, DynamicStatisticTask>,
}

enum FileStatisticTask {
    Mine {
        task: SummaryStatisticTaskPtr
    },
    Proxy {
        task: SummaryStatisticTaskPtr, 
        task_cb: StatisticTaskPtr
    },
}

impl FileStatisticTask {
    fn new(task_cb: Option<StatisticTaskPtr>) -> Self {
        let task = SummaryStatisticTaskImpl::new(Some(DynamicStatisticTask::default().ptr())).ptr();
        
        if let Some(task_cb) = task_cb {
            Self::Proxy {
                task, 
                task_cb
            }
        } else {
            FileStatisticTask::Mine { 
                task
            }
        }
    }

    fn as_task(&self) -> &dyn SummaryStatisticTask {
        match self {
            Self::Mine { task } => task.as_ref(), 
            Self::Proxy { task, .. } => task.as_ref()
        } 
    } 
}

impl std::fmt::Display for FileStatisticTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Mine{task} => {
                write!(f, "file mine task = {}", task)
            },
            Self::Proxy{task, task_cb: _} => {
                write!(f, "file proxy task = {}", task)
            }
        }
    }
}

struct TaskImpl {
    stack: WeakStack, 
    file: File,
    chunk_list: ChunkListDesc, 
    ranges: Vec<(usize, Option<Range<u64>>)>, 
    config: Arc<ChunkDownloadConfig>, 
    resource: ResourceManager, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box<dyn ChunkWriterExt>>,
    statistic_task: FileStatisticTask
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
        task_cb: Option<StatisticTaskPtr>) -> Self {
        let chunk_list = chunk_list.unwrap_or(ChunkListDesc::from_file(&file).unwrap());
        let statistic_task = FileStatisticTask::new(task_cb);
        statistic_task.as_task().add_total_size(file.desc().content().len());

        Self(Arc::new(TaskImpl {
            stack, 
            file: file.clone(), 
            ranges: (0..chunk_list.chunks().len()).into_iter().map(|i| (i, None)).collect(),  
            chunk_list, 
            config, 
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0),
                chunk_statistic_task: BTreeMap::new(),
            }), 
            writers: writers.into_iter().map(|w| ChunkWriterExtWrapper::new(w).clone_as_writer()).collect(),
            statistic_task
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
        task_cb: Option<StatisticTaskPtr>) -> Self {
        
        let chunk_list = chunk_list.unwrap_or(ChunkListDesc::from_file(&file).unwrap());
        let statistic_task = FileStatisticTask::new(task_cb);
        let ranges = if ranges.is_none() {
            statistic_task.as_task().add_total_size(file.desc().content().len());
            (0..chunk_list.chunks().len()).into_iter().map(|i| (i, None)).collect()
        } else {
            let ranges = ranges.unwrap();
            let mut dst_ranges = vec![];
            let mut total_len = 0; 
            for range in ranges {
                for (index, range) in chunk_list.range_of(range) {
                    total_len += range.end - range.start;
                    dst_ranges.push((index, Some(range)));
                }   
            } 
            statistic_task.as_task().add_total_size(total_len);
            dst_ranges
        };

        Self(Arc::new(TaskImpl {
            stack, 
            file: file.clone(), 
            chunk_list, 
            ranges, 
            config, 
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0),
                chunk_statistic_task: BTreeMap::new(),
            }), 
            writers,
            statistic_task
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

    pub fn config(&self) -> &Arc<ChunkDownloadConfig> {
        &self.0.config
    }

    pub fn as_statistic(&self) -> Arc<dyn StatisticTask> {
        Arc::from(self.clone())
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
                            self.resource().clone(),
                            Some(self.as_statistic())
                        );
                        downloading.cur_index = next_index;
                        downloading.cur_task = chunk_task.clone();

                        if let Some(s) = chunk_task.statistic_task() {
                            let _ = state.chunk_statistic_task.
                                          entry(chunk_task.chunk().clone()).
                                          or_insert(s);
                        }

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
            state.control_state = TaskControlState::Finished;

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
        self.reset();

        if self.ranges().len() > 0 {
            let (index, range) = self.ranges()[0].clone();
            let chunk_task = ChunkTask::with_range(
                self.0.stack.clone(), 
                self.chunk_list().chunks()[index].clone(),
                range,  
                self.config().clone(), 
                vec![self.clone_as_writer()], 
                self.resource().clone(),
                Some(self.as_statistic())
            );
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
            if let Some(s) = chunk_task.statistic_task() {
                let _ = state.chunk_statistic_task.
                              entry(chunk_task.chunk().clone()).
                              or_insert(s);
            }

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
                state.control_state = TaskControlState::Finished;
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

impl StatisticTask for FileTask {
    fn reset(&self) {
        match &self.0.statistic_task {
            FileStatisticTask::Mine{task} => {
                task.reset();
            }
            FileStatisticTask::Proxy{task, task_cb: _} => {
                task.reset();
            }
        }
    }


    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        match &self.0.statistic_task {
            FileStatisticTask::Mine{task} => {
                let r = task.on_stat(size).unwrap();

                let mut state = self.0.state.write().unwrap();
                match state.control_state {
                    TaskControlState::Downloading(_, _) => {
                        state.control_state = TaskControlState::Downloading(r.bandwidth() as usize, task.progress());
                    }
                    _ => {}
                }

                Ok(r)
            }
            FileStatisticTask::Proxy{task, task_cb} => {
                let _ = task.on_stat(size).unwrap();

                task_cb.on_stat(size)
            }
        }
    }

}