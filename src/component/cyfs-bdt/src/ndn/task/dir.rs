use std::{
    sync::RwLock, 
    collections::LinkedList,
    vec::Vec,
};
use async_std::{
    sync::Arc, 
    task
};
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{Stack, WeakStack}
};
use super::super::{
    chunk::*, 
    scheduler::*, 
};
use super::{
    chunk::ChunkTask, 
    chunk_list::ChunkListTask, 
    file::FileTask,
    statistic::{SummaryStatisticTaskPtr, SummaryStatisticTaskImpl}

};

const TASK_COUNT_MAX_DEFAULT: usize = 5;

#[derive(Clone)]
pub struct Config { 
    pub task_count_max: usize,
}

impl std::default::Default for Config {
    fn default() -> Self {
        Self {
            task_count_max: TASK_COUNT_MAX_DEFAULT,
        }
    }
   
}

// 因为按路径加入可能出现同一个object被多次下载的情况；
// 所以需要一个单独的id 而不是 object id标识 sub task
#[derive(Clone)]
enum SubTask {
    ChunkList(IncreaseId, ChunkListTask),
    Chunk(IncreaseId, ChunkTask), 
    File(IncreaseId, FileTask), 
    Dir(IncreaseId, DirTask), 
    End
}

impl std::fmt::Display for SubTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChunkList(id, task) => write!(f, "DirSubTask:{{id:{}, task:{}}}", id, task), 
            Self::Chunk(id, task) => write!(f, "DirSubTask:{{id:{}, task:{}}}", id, task), 
            Self::File(id, task) => write!(f, "DirSubTask:{{id:{}, task:{}}}", id, task), 
            Self::Dir(id, task) => write!(f, "DirSubTask:{{id:{}, task:{}}}", id, task),
            Self::End => write!(f, "DirEndSubTask") 
        }
    }
}


impl SubTask {
    fn id(&self) -> IncreaseId {
        match self {
            Self::ChunkList(id, _) => *id,
            Self::Chunk(id, _) => *id, 
            Self::File(id, _) => *id, 
            Self::Dir(id, _) => *id,
            Self::End => IncreaseId::invalid() 
        }
    }

    fn is_end(&self) -> bool {
        if let Self::End = self {
            true
        } else {
            false
        }
    }

    fn as_task(&self) -> Option<&dyn DownloadTask> {
        match self {
            Self::ChunkList(_, task) => Some(task),
            Self::Chunk(_, task) => Some(task),
            Self::File(_, task) => Some(task), 
            Self::Dir(_, task) => Some(task), 
            _ => None
        }
    }

    fn total_len(&self) -> Option<u64> {
        match self {
            Self::ChunkList(_, task) => Some(task.chunk_list().total_len()),
            Self::Chunk(_, task) => Some(task.chunk().len() as u64),
            Self::File(_, task) => Some(task.file().desc().content().len()),
            Self::Dir(_, _) | _ => None,
        }
    }
}

enum TaskStateImpl {
    Downloading(LinkedList<SubTask>),
    Finished, 
    Canceled(BuckyErrorCode)
}

struct StateImpl {
    schedule_state: TaskStateImpl, 
    downloading_state: Vec<SubTask>,
    downloaded_state: Vec<SubTask>,
    control_state: TaskControlState
}

struct TaskImpl {
    stack: WeakStack, 
    dir: DirId, 
    config: Arc<ChunkDownloadConfig>, 
    resource: ResourceManager, 
    sub_id: IncreaseIdGenerator, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box<dyn ChunkWriter>>,
    statistic_task: SummaryStatisticTaskPtr,
}

pub trait DirTaskControl: Send + Sync {
    // 如果dir meta比较大，包含很多chunk，在dir body中的meta是 chunk id list；首先需要下载meta chunks；
    // 完整加入dir meta的chunk list；dir task进入DownloadingMeta状态；标准的chunk writer的一般实现是写入到内存中展开meta数据；
    fn add_meta(&self, chunk_list: ChunkListDesc, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<()>;
    
    // 当本地已有meta 数据后，上层按照dir 的组织形式，在合适的时机组合以下调用，按需添加dir task的sub task;
    // 压缩多个path 到 一个chunk，产生chunk sub task
    fn add_chunk(&self, chunk: ChunkId, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<()>;
    // 产生file sub task
    fn add_file(&self, file: File, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<()>;
    // 嵌套子目录，产生 dir sub task；返回的dir sub task实例；
    fn add_dir(&self, dir: DirId, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<Box<dyn DirTaskControl>>;
    // 所有路径的sub task已经加入到dir task后，调用finish；当dir task的sub task全部完成后，dir task进入finish 状态
    fn finish(&self) -> BuckyResult<()>;
}

#[derive(Clone)]
pub struct DirTask(Arc<TaskImpl>);

impl std::fmt::Display for DirTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DirTask::{{dir:{}}}",  self.dir_id())
    }
}

impl DirTask {
    pub fn new(stack: WeakStack, 
        dir: DirId, 
        config: Arc<ChunkDownloadConfig>, 
        writers: Vec<Box<dyn ChunkWriter>>,
        owner: ResourceManager,
        task_cb: Option<StatisticTaskPtr>) -> Self {
        Self (Arc::new(TaskImpl {
            stack: stack,
            dir: dir,
            config: config,
            resource: owner,
            sub_id: IncreaseIdGenerator::new(),
            state: RwLock::new(StateImpl{
                schedule_state: TaskStateImpl::Downloading(LinkedList::new()),
                downloading_state: Vec::new(),
                downloaded_state: Vec::new(),
                control_state: TaskControlState::Downloading(0, 0),
            }),
            writers: writers,
            statistic_task: 
                {
                    if let Some(cb) = task_cb {
                        SummaryStatisticTaskImpl::new(Some(cb)).ptr()
                    } else {
                        SummaryStatisticTaskImpl::new(Some(DynamicStatisticTask::default().ptr())).ptr()
                    }
                }
        }))
    }
    
    pub fn dir_id(&self) -> &DirId {
        &self.0.dir
    }

    pub fn config(&self) -> &Arc<ChunkDownloadConfig> {
        &self.0.config
    }

    pub fn as_statistic(&self) -> StatisticTaskPtr {
        Arc::from(self.clone())
    }

    fn cancel_by_error(&self, _id: IncreaseId, _err: BuckyErrorCode) -> BuckyResult<()> {
        Ok(())
    }

    fn on_sub_task_error(&self, _id: IncreaseId, _err: BuckyErrorCode) {
    }

    fn on_sub_task_finish(&self, id: IncreaseId) {
        {
            let mut state = self.0.state.write().unwrap();

            for i in 0..state.downloading_state.len() {
                let downloading_task = &state.downloading_state[i];

                if downloading_task.id() == id {
                    let downloading_task = state.downloading_state.remove(i);
                    state.downloaded_state.push(downloading_task.clone());
                    break;
                }
            }
        }

        match self.start() {
            TaskState::Finished => {
                let task = self.clone();
                task::spawn(async move {
                    for w in &task.0.writers {
                        let _ = w.finish().await;
                    }
                });
            }
            _ => {}
        }

    }

    fn on_sub_task_post_finish(&self, _id: IncreaseId) {
    }

    fn add_sub_task(&self, task: SubTask) -> BuckyResult<()> {
        self.add_sub_task_inner(task.clone())
            .map(|start| {
                if start {
                    if let Some(size) = task.total_len() {
                        self.0.statistic_task.add_total_size(size);
                    }
                    let _ = self.start();
                }
            })
            .map_err(|err| {
                if let Some(task) = task.as_task() {
                    let _ = self.resource().remove_child(task.resource());
                }
                err
            })
    }

    fn add_sub_task_inner(&self, task: SubTask) -> BuckyResult<bool> {
        let mut state = self.0.state.write().unwrap();

        match &mut state.schedule_state {
            TaskStateImpl::Downloading(tasks) => {
                if let Some(back) = tasks.back() {
                    if back.is_end() {
                        return Err(BuckyError::new(BuckyErrorCode::ErrorState, "task is waiting finish."));
                    }
                }

                tasks.push_back(task.clone());

                return Ok(true);
            }
            _ => {
                error!("{} ignore sub {} for not pending/downloading", self, task);
                Err(BuckyError::new(BuckyErrorCode::ErrorState, "not pending/downloading"))
            }
        }
    }

    fn meta_writer(&self, 
                   id: IncreaseId, 
                   writers: Vec<Box<dyn ChunkWriter>>) -> Box<dyn ChunkWriter> {
        struct MetaWriterImpl {
            dir: DirTask, 
            id: IncreaseId, 
            writers: Vec<Box<dyn ChunkWriter>>
        }

        #[derive(Clone)]
        struct MetaWriter(Arc<MetaWriterImpl>);

        impl std::fmt::Display for MetaWriter {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "MetaWriter::{{dir:{}}}",  self.0.dir.dir_id())
            }
        }

        #[async_trait::async_trait]
        impl ChunkWriter for MetaWriter {
            fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
                Box::new(self.clone())
            }

            async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
                for w in &self.0.writers {
                    let _ = w.write(chunk, content.clone()).await;
                }
                Ok(())
            }

            async fn redirect(&self, redirect_node: &DeviceId) -> BuckyResult<()> {
                for writer in self.0.writers.iter() {
                    let _ = writer.redirect(redirect_node).await;
                }
                Ok(())
            }
        
            async fn finish(&self) -> BuckyResult<()> {
                self.0.dir.on_sub_task_finish(self.0.id);

                for w in &self.0.writers {
                    let _ = w.finish().await;
                }

                Ok(())
            }

            async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()> {
                for w in &self.0.writers {
                    let _ = w.err(e).await;
                }

                // self.0.dir.on_sub_task_error(self.0.id, e);
                // 完成此次任务，不结束整体任务 
                self.0.dir.on_sub_task_finish(self.0.id);

                Ok(())
            }
        }

        Box::new(MetaWriter(Arc::new(MetaWriterImpl { dir: self.clone(), id, writers })))
    }

    fn sub_writer(
        &self, 
        sub_id: IncreaseId, 
        writers: Vec<Box<dyn ChunkWriter>>) -> Box<dyn ChunkWriter> {
        struct WriterImpl {
            dir: DirTask, 
            id: IncreaseId, 
            writers: Vec<Box<dyn ChunkWriter>>
        }

        #[derive(Clone)]
        struct Writer(Arc<WriterImpl>);

        impl std::fmt::Display for Writer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "FileWriter::{{dir:{}, id:{}}}",  self.0.dir.dir_id(), self.0.id)
            }
        } 

        #[async_trait::async_trait]
        impl ChunkWriter for Writer {
            fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
                Box::new(self.clone())
            }

            async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
                for w in &self.0.writers {
                    let _ = w.write(chunk, content.clone()).await;
                }
                Ok(())
            }

            async fn finish(&self) -> BuckyResult<()> {
                self.0.dir.on_sub_task_finish(self.0.id);

                for w in &self.0.writers {
                    let _ = w.finish().await;
                }

                Ok(())
            }

            async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()> {
                for w in &self.0.writers {
                    let _ = w.err(e).await;
                }
                self.0.dir.on_sub_task_finish(self.0.id);
                Ok(())
            }

            async fn redirect(&self, redirect_node: &DeviceId) -> BuckyResult<()> {
                for w in &self.0.writers {
                    let _ = w.redirect(redirect_node).await;
                }
                self.0.dir.on_sub_task_finish(self.0.id);
                Ok(())
            }

        }

        Box::new(Writer(Arc::new(WriterImpl {
            dir: self.clone(), 
            id: sub_id, 
            writers
        })))
    }

}

impl DirTaskControl for DirTask {
    fn add_meta(&self, chunk_list: ChunkListDesc, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<()> {
        let sub_id = self.0.sub_id.generate();
        let task = ChunkListTask::new(self.0.stack.clone(),
                                                    format!("DirMeta:{}", self.dir_id()).to_owned(),
                                                    chunk_list,
                                                    self.0.config.clone(),
                                                    vec![self.meta_writer(sub_id.clone(), writers)],
                                                    self.resource().clone(),
                                                    Some(self.as_statistic()));

        self.add_sub_task(SubTask::ChunkList(sub_id.clone(), task))
    }

    fn add_chunk(&self, chunk: ChunkId, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<()> {
        let sub_id = self.0.sub_id.generate();
        let chunk_task = ChunkTask::new(self.0.stack.clone(), 
                                                   chunk, 
                                                   self.config().clone(), 
                                                   vec![self.sub_writer(sub_id.clone(), writers)], 
                                                   self.resource().clone(),
                                                   Some(self.as_statistic()));

        self.add_sub_task(SubTask::Chunk(sub_id.clone(), chunk_task))
    }

    fn add_file(&self, file: File, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<()> {
        let sub_id = self.0.sub_id.generate();
        let file_task = FileTask::new(self.0.stack.clone(), 
                                                file, 
                                                None, 
                                                self.config().clone(), 
                                                vec![self.sub_writer(sub_id.clone(), writers)],
                                                self.resource().clone(),
                                                Some(self.as_statistic()));
        self.add_sub_task(SubTask::File(sub_id.clone(), file_task))
    }

    fn add_dir(&self, dir: DirId, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<Box<dyn DirTaskControl>> {
        let sub_id = self.0.sub_id.generate();
        let dir_task = Self::new(self.0.stack.clone(), 
                                          dir, 
                                          self.config().clone(), 
                                          vec![self.sub_writer(sub_id, writers)],
                                          self.resource().clone(),
                                          Some(self.as_statistic()));
    
        self.add_sub_task(SubTask::Dir(sub_id.clone(), dir_task.clone()))
            .map(|_| Box::new(dir_task) as Box<dyn DirTaskControl>)
    }

    fn finish(&self) -> BuckyResult<()> {
        self.add_sub_task(SubTask::End)
    }
}


impl TaskSchedule for DirTask {
    fn schedule_state(&self) -> TaskState {
        match &self.0.state.read().unwrap().schedule_state {
            TaskStateImpl::Downloading(_) => TaskState::Running(0), 
            TaskStateImpl::Finished => TaskState::Finished, 
            TaskStateImpl::Canceled(err) => TaskState::Canceled(*err)
        }
    }

    fn resource(&self) -> &ResourceManager {
        &self.0.resource
    }

    //保证不会重复调用
    fn start(&self) -> TaskState {
        enum NextStep {
            Content(Box<dyn DownloadTask>),
            Pending,
            Finish,
        }

        let step = {
            let dir_config = Stack::from(&self.0.stack).config().ndn.dir.clone();

            let mut state = self.0.state.write().unwrap();

            if state.downloading_state.len() >= dir_config.task_count_max {
                NextStep::Pending
            } else {
                match &mut state.schedule_state {
                    TaskStateImpl::Downloading(content) => {
                        if let Some(task) = content.front() {
                            if task.is_end() {
                                if state.downloading_state.len() > 0 {
                                    NextStep::Pending
                                } else {
                                    NextStep::Finish
                                }
                            } else {
                                if let Some(task) = content.pop_front() {
                                    state.downloading_state.push(task.clone());
                                    if let Some(task_impl) = task.as_task() {
                                        NextStep::Content(task_impl.clone_as_download_task())
                                    } else {
                                        NextStep::Pending
                                    }
                                } else {
                                    unreachable!();
                                }
                            }
                        } else {
                            NextStep::Pending
                        }
                    },
                    _ => {
                        NextStep::Finish
                    }
                }
            }
        };

        match step {
            NextStep::Content(task) => {
                task.start()
            },
            NextStep::Pending => TaskState::Pending,
            _ =>{
                self.0.state.write().unwrap().control_state = TaskControlState::Finished;
                TaskState::Finished
            }
        }
    }
}

impl DownloadTaskControl for DirTask {
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

impl DownloadTask for DirTask {
    fn clone_as_download_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }
}

impl StatisticTask for DirTask {
    fn reset(&self) {
        self.0.statistic_task.reset()
    }

    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        let r = self.0.statistic_task.on_stat(size).unwrap();

        {
            let mut state = self.0.state.write().unwrap();
            match state.control_state {
                TaskControlState::Downloading(_, _) => {
                    state.control_state = TaskControlState::Downloading(r.bandwidth() as usize, self.0.statistic_task.progress())
                }
                _ => {}
            }
        }

        Ok(r)
    }

}

