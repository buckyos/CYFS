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
};
use super::{
    common::*, 
    chunk::ChunkTask, 
    chunk_list::ChunkListTask, 
    file::FileTask,
};



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

    fn as_task(&self) -> Option<&dyn DownloadTask2> {
        match self {
            Self::ChunkList(_, task) => Some(task),
            Self::Chunk(_, task) => Some(task),
            Self::File(_, task) => Some(task), 
            Self::Dir(_, task) => Some(task), 
            _ => None
        }
    }

    fn clone_as_task(&self) -> Box<dyn DownloadTask2> {
        self.as_task().unwrap().clone_as_task()
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
    Downloading(DownloadingState),
    Finished, 
    Error(BuckyErrorCode)
}

struct DownloadingState {
    pending_tasks: LinkedList<SubTask>, 
    cur_task: Option<SubTask>, 
    history_speed: HistorySpeed, 
}

struct StateImpl {
    schedule_state: TaskStateImpl, 
    control_state: DownloadTaskControlState
}

struct TaskImpl {
    stack: WeakStack, 
    dir: DirId, 
    context: SingleDownloadContext, 
    sub_id: IncreaseIdGenerator, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box<dyn ChunkWriter>>,
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
    pub fn new(
        stack: WeakStack, 
        dir: DirId, 
        context: SingleDownloadContext, 
        writers: Vec<Box<dyn ChunkWriter>>,
    ) -> Self {
        let strong_stack = Stack::from(&stack);
        Self (Arc::new(TaskImpl {
            stack: stack,
            dir: dir,
            context, 
            sub_id: IncreaseIdGenerator::new(),
            state: RwLock::new(StateImpl{
                schedule_state: TaskStateImpl::Downloading(DownloadingState {
                    pending_tasks: LinkedList::new(), 
                    cur_task: None, 
                    history_speed: HistorySpeed::new(0, strong_stack.config().ndn.channel.history_speed.clone()), 
                }),
                control_state: DownloadTaskControlState::Normal,
            }),
            writers: writers,
        }))
    }
    
    pub fn dir_id(&self) -> &DirId {
        &self.0.dir
    }

    pub fn context(&self) -> &SingleDownloadContext  {
        &self.0.context
    }

    fn cancel_by_error(&self, _id: IncreaseId, _err: BuckyErrorCode) -> BuckyResult<()> {
        Ok(())
    }

    fn on_sub_task_error(&self, _id: IncreaseId, _err: BuckyErrorCode) {
    }

    fn on_sub_task_finish(&self, id: IncreaseId) {
        if let Some(expect_speed) = {
            let mut state = self.0.state.write().unwrap();

            match &mut state.schedule_state {
                TaskStateImpl::Downloading(downloading) => {
                    if let Some(cur_task) = &downloading.cur_task {
                        if cur_task.id() == id {
                            downloading.cur_task = None;
                            Some(downloading.history_speed.average())
                        } else {
                            None
                        } 
                    } else {
                        None
                    }
                },
                _ => None
            }
        } {
            let _ = self.on_drain(expect_speed);
        }

    }

    fn on_sub_task_post_finish(&self, _id: IncreaseId) {
    }

    fn add_sub_task(&self, task: SubTask) -> BuckyResult<()> {
        self.add_sub_task_inner(task.clone())
            .map(|start| {
                if start {
                    let _ = self.on_drain(0);
                }
            })
    }

    fn add_sub_task_inner(&self, task: SubTask) -> BuckyResult<bool> {
        let mut state = self.0.state.write().unwrap();

        match &mut state.schedule_state {
            TaskStateImpl::Downloading(downloading) => {
                if let Some(back) = downloading.pending_tasks.back() {
                    if back.is_end() {
                        return Err(BuckyError::new(BuckyErrorCode::ErrorState, "task is waiting finish."));
                    }
                }

                downloading.pending_tasks.push_back(task.clone());

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
                                        self.context().clone(),
                                        vec![self.meta_writer(sub_id.clone(), writers)]
                                    );

        self.add_sub_task(SubTask::ChunkList(sub_id.clone(), task))
    }

    fn add_chunk(&self, chunk: ChunkId, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<()> {
        let sub_id = self.0.sub_id.generate();
        let chunk_task = ChunkTask::new(self.0.stack.clone(), 
                                                   chunk, 
                                                   self.context().clone(), 
                                                   vec![self.sub_writer(sub_id.clone(), writers)]
                                                );

        self.add_sub_task(SubTask::Chunk(sub_id.clone(), chunk_task))
    }

    fn add_file(&self, file: File, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<()> {
        let sub_id = self.0.sub_id.generate();
        let file_task = FileTask::new(self.0.stack.clone(), 
                                                file, 
                                                None, 
                                                self.context().clone(), 
                                                vec![self.sub_writer(sub_id.clone(), writers)]
                                            );
        self.add_sub_task(SubTask::File(sub_id.clone(), file_task))
    }

    fn add_dir(&self, dir: DirId, writers: Vec<Box<dyn ChunkWriter>>) -> BuckyResult<Box<dyn DirTaskControl>> {
        let sub_id = self.0.sub_id.generate();
        let dir_task = Self::new(self.0.stack.clone(), 
                                          dir, 
                                          self.context().clone(), 
                                          vec![self.sub_writer(sub_id, writers)]
                                        );
    
        self.add_sub_task(SubTask::Dir(sub_id.clone(), dir_task.clone()))
            .map(|_| Box::new(dir_task) as Box<dyn DirTaskControl>)
    }

    fn finish(&self) -> BuckyResult<()> {
        self.add_sub_task(SubTask::End)
    }
}


impl DownloadTask2 for DirTask {
    fn clone_as_task(&self) -> Box<dyn DownloadTask2> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        match &self.0.state.read().unwrap().schedule_state {
            TaskStateImpl::Downloading(_) => DownloadTaskState::Downloading(0, 0.0), 
            TaskStateImpl::Finished => DownloadTaskState::Finished, 
            TaskStateImpl::Error(err) => DownloadTaskState::Error(*err)
        }
    }


    fn control_state(&self) -> DownloadTaskControlState {
        self.0.state.read().unwrap().control_state.clone()
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.state.write().unwrap();

        if let TaskStateImpl::Downloading(downloading) = &mut state.schedule_state {
            let cur_speed = if let Some(cur_task) = &downloading.cur_task {
                cur_task.as_task().unwrap().calc_speed(when)
            } else {
                0
            };
            downloading.history_speed.update(Some(cur_speed), when);
            cur_speed
        } else {
            0
        }
        
    }

    fn cur_speed(&self) -> u32 {
        if let Some(task) = {
            let state = self.0.state.read().unwrap();

            if let TaskStateImpl::Downloading(downloading) = &state.schedule_state {
                downloading.cur_task.as_ref().map(|t| t.clone_as_task())
            } else {
                None
            }
        } {
            task.cur_speed()
        } else {
            0
        }
    }

    fn history_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();

        if let TaskStateImpl::Downloading(downloading) = &state.schedule_state {
            downloading.history_speed.average()
        } else {
            0
        }
    }

    fn drain_score(&self) -> i64 {
        if let Some(task) = {
            let state = self.0.state.read().unwrap();

            if let TaskStateImpl::Downloading(downloading) = &state.schedule_state {
                downloading.cur_task.as_ref().map(|t| t.clone_as_task())
            } else {
                None
            }
        } {
            task.drain_score()
        } else {
            0
        }
    }

    //保证不会重复调用
    fn on_drain(&self, expect_speed: u32) -> u32 {
        enum NextStep {
            Content(Box<dyn DownloadTask2>),
            Pending,
            Finish,
        }

        let step = {
            let mut state = self.0.state.write().unwrap();           
            match &mut state.schedule_state {
                TaskStateImpl::Downloading(downloading) => {
                    if let Some(cur_task) = &downloading.cur_task {
                        NextStep::Content(cur_task.clone_as_task())
                    } else {
                        if let Some(task) = downloading.pending_tasks.front() {
                            if task.is_end() {
                                NextStep::Finish
                            } else {
                                if let Some(task) = downloading.pending_tasks.pop_front() {
                                    downloading.cur_task = Some(task.clone());
                                    NextStep::Content(task.clone_as_task())
                                } else {
                                    unreachable!()
                                }
                            }
                        } else {
                            NextStep::Pending
                        }
                    }
                },
                _ => {
                    NextStep::Finish
                }
            }
            
        };

        match step {
            NextStep::Content(task) => task.on_drain(expect_speed),
            NextStep::Pending => 0, 
            NextStep::Finish => {
                self.0.state.write().unwrap().schedule_state = TaskStateImpl::Finished;

                let task = self.clone();
                task::spawn(async move {
                    for w in &task.0.writers {
                        let _ = w.finish().await;
                    }
                });
                0
            }
        }
    }
}


