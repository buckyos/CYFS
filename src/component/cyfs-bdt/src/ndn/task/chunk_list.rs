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
    schedule_state: TaskStateImpl
}

enum ChunklistStatisticTask {
    Mine {
        task: SummaryStatisticTaskPtr
    },
    Proxy {
        task: SummaryStatisticTaskPtr, 
        task_cb: StatisticTaskPtr
    },
}

impl ChunklistStatisticTask {
    fn new(task_cb: Option<StatisticTaskPtr>) -> Self {
        let task = SummaryStatisticTaskImpl::new(Some(DynamicStatisticTask::default().ptr())).ptr();
        
        if let Some(task_cb) = task_cb {
            Self::Proxy {
                task, 
                task_cb
            }
        } else {
            Self::Mine { 
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

impl std::fmt::Display for ChunklistStatisticTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Mine{task} => {
                write!(f, "chunk-list mine task = {}", task)
            },
            Self::Proxy{task, task_cb: _} => {
                write!(f, "chunk-list proxy task = {}", task)
            }
        }
    }
}

struct TaskImpl {
    stack: WeakStack, 
    name: String, 
    chunk_list: ChunkListDesc, 
    ranges: Vec<(usize, Option<Range<u64>>)>, 
    config: Arc<ChunkDownloadConfig>, 
    resource: ResourceManager, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box<dyn ChunkWriterExt>>,
    statistic_task_cb: ChunklistStatisticTask,
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
        config: Arc<ChunkDownloadConfig>, 
        writers: Vec<Box <dyn ChunkWriter>>, 
        owner: ResourceManager,
        task_cb: Option<StatisticTaskPtr>) -> Self {

        let task = ChunklistStatisticTask::new(task_cb);
        task.as_task().add_total_size(chunk_list.total_len());

        Self(Arc::new(TaskImpl {
            stack, 
            name, 
            ranges: (0..chunk_list.chunks().len()).into_iter().map(|i| (i, None)).collect(), 
            chunk_list, 
            config, 
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0)
            }), 
            writers: writers.into_iter().map(|w| ChunkWriterExtWrapper::new(w).clone_as_writer()).collect(),
            statistic_task_cb: task
        }))
    } 

    pub fn with_ranges(
        stack: WeakStack,
        name: String, 
        chunk_list: ChunkListDesc, 
        ranges: Option<Vec<Range<u64>>>, 
        config: Arc<ChunkDownloadConfig>, 
        writers: Vec<Box <dyn ChunkWriterExt>>, 
        owner: ResourceManager,
        task_cb: Option<StatisticTaskPtr>) -> Self {

        let task = ChunklistStatisticTask::new(task_cb);
        task.as_task().add_total_size(chunk_list.total_len());
    
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
            config, 
            resource: ResourceManager::new(Some(owner)), 
            state: RwLock::new(StateImpl {
                schedule_state: TaskStateImpl::Pending, 
                control_state: TaskControlState::Downloading(0, 0)
            }), 
            writers,
            statistic_task_cb: task
        }))
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

    fn as_statistic(&self) -> StatisticTaskPtr {
        Arc::from(self.clone())
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

    async fn redirect(&self, redirect_node: &DeviceId) -> BuckyResult<()> {
        {
            let state = &mut *self.0.state.write().unwrap();
            state.control_state = TaskControlState::Redirect(redirect_node.clone());
        }

        for writer in self.0.writers.iter() {
            let _ = writer.redirect(redirect_node).await;
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
                            None,
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
            state.control_state = TaskControlState::Finished;
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
                self.config().clone(), 
                vec![self.clone_as_writer()], 
                self.resource().clone(),
                Some(self.as_statistic()));
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
                state.control_state = TaskControlState::Finished;
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

impl StatisticTask for ChunkListTask {
    fn reset(&self) {
        match &self.0.statistic_task_cb {
            ChunklistStatisticTask::Mine{task} => {
                task.reset();
            }
            ChunklistStatisticTask::Proxy{task, task_cb: _} => {
                task.reset();
            }
        }

    }

    // fn stat(&self) -> BuckyResult<Box<dyn PerfDataAbstract>> {
    //     if let Some(task) = &self.0.statistic_task_cb {
    //         task.stat()
    //     } else {
    //         Ok(PerfData::default().clone_as_perfdata())
    //     }
    // }

    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        match &self.0.statistic_task_cb {
            ChunklistStatisticTask::Mine{task} => {
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
            ChunklistStatisticTask::Proxy{task, task_cb} => {
                let _ = task.on_stat(size).unwrap();

                task_cb.on_stat(size)
            }
        // if let Some(task) = &self.0.statistic_task_cb {
        //     task.on_stat(size)
        // } else {
        //     Ok(PerfData::default().clone_as_perfdata())
        // }
        }
    }
}