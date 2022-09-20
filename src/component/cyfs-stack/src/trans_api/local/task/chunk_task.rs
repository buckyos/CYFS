use super::super::download_task_manager::DownloadTaskState;
use crate::ndn_api::{ChunkManagerWriter, LocalChunkWriter};
use crate::trans_api::TransStore;
use cyfs_chunk_cache::ChunkManager;
use cyfs_base::*;
use cyfs_bdt::{
    self, 
    SingleDownloadContext, 
    ChunkWriter, 
    StackGuard, 
};
use cyfs_task_manager::*;

use sha2::Digest;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct DownloadChunkTask {
    task_id: TaskId,
    chunk_id: ChunkId,
    bdt_stack: StackGuard,
    device_list: Vec<DeviceId>,
    referer: String,
    context_id: Option<ObjectId>,
    session: async_std::sync::Mutex<Option<Box<dyn cyfs_bdt::DownloadTask>>>,
    writer: Box<dyn ChunkWriter>,
    task_store: Option<Arc<dyn TaskStore>>,
    task_status: Mutex<TaskStatus>,
}

impl DownloadChunkTask {
    pub(crate) fn new(
        chunk_id: ChunkId,
        bdt_stack: StackGuard,
        device_list: Vec<DeviceId>,
        referer: String,
        context_id: Option<ObjectId>,
        task_label_data: Vec<u8>,
        writer: Box<dyn ChunkWriter>,
    ) -> Self {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(chunk_id.as_slice());
        sha256.input(task_label_data.as_slice());
        let task_id = sha256.result().into();
        Self {
            task_id,
            chunk_id,
            bdt_stack,
            device_list,
            referer,
            context_id,
            session: async_std::sync::Mutex::new(None),
            writer,
            task_store: None,
            task_status: Mutex::new(TaskStatus::Stopped),
        }
    }
}

#[async_trait::async_trait]
impl Task for DownloadChunkTask {
    fn get_task_id(&self) -> TaskId {
        self.task_id.clone()
    }

    fn get_task_type(&self) -> TaskType {
        DOWNLOAD_CHUNK_TASK
    }

    fn get_task_category(&self) -> TaskCategory {
        DOWNLOAD_TASK_CATEGORY
    }

    async fn get_task_status(&self) -> TaskStatus {
        *self.task_status.lock().unwrap()
    }

    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>) {
        self.task_store = Some(task_store);
    }

    async fn start_task(&self) -> BuckyResult<()> {
        let mut session = self.session.lock().await;
        // if session.is_some() {
        //     session.as_ref().unwrap().resume()?;
        //     return Ok(());
        // }

        {
            if *self.task_status.lock().unwrap() == TaskStatus::Running {
                return Ok(());
            }
        }

        let context = SingleDownloadContext::streams(
            if self.referer.len() > 0 {
                Some(self.referer.to_owned())
            } else {
                None
            }, 
            vec![self.device_list[0].clone()]);
        
        // 创建bdt层的传输任务
        *session = Some(
            cyfs_bdt::download::download_chunk(
                &self.bdt_stack,
                self.chunk_id.clone(), 
                None, 
                Some(context),
                vec![self.writer.clone_as_writer()],
            )
            .await
            .map_err(|e| {
                error!(
                    "start bdt chunk trans session error! task_id={}, {}",
                    self.task_id.to_string(),
                    e
                );
                e
            })?,
        );

        info!(
            "create bdt chunk trans session success: task={}, device={:?}",
            self.task_id.to_string(),
            self.device_list,
        );
        *self.task_status.lock().unwrap() = TaskStatus::Running;
        self.task_store
            .as_ref()
            .unwrap()
            .save_task_status(&self.task_id, TaskStatus::Running)
            .await?;

        Ok(())
    }

    async fn pause_task(&self) -> BuckyResult<()> {
        let session = self.session.lock().await;
        if session.is_some() {
            session.as_ref().unwrap().pause()?;
        }
        *self.task_status.lock().unwrap() = TaskStatus::Paused;
        self.task_store
            .as_ref()
            .unwrap()
            .save_task_status(&self.task_id, TaskStatus::Paused)
            .await?;
        Ok(())
    }

    async fn stop_task(&self) -> BuckyResult<()> {
        let session = {
            let mut session = self.session.lock().await;
            if session.is_none() {
                return Ok(());
            } else {
                session.take().unwrap()
            }
        };

        session.cancel()?;

        *self.task_status.lock().unwrap() = TaskStatus::Stopped;
        self.task_store
            .as_ref()
            .unwrap()
            .save_task_status(&self.task_id, TaskStatus::Stopped)
            .await?;
        Ok(())
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        let session = self.session.lock().await;
        let task_state = if session.is_some() {
            let state = session.as_ref().unwrap().state();
            match state {
                cyfs_bdt::DownloadTaskState::Downloading(speed, progress) => DownloadTaskState {
                    task_status: TaskStatus::Running,
                    err_code: None,
                    speed: speed as u64,
                    upload_speed: 0,
                    downloaded_progress: progress as u64,
                    sum_size: self.chunk_id.len() as u64,
                },
                cyfs_bdt::DownloadTaskState::Paused => DownloadTaskState {
                    task_status: TaskStatus::Paused,
                    err_code: None,
                    speed: 0,
                    upload_speed: 0,
                    downloaded_progress: 0,
                    sum_size: self.chunk_id.len() as u64,
                },
                cyfs_bdt::DownloadTaskState::Error(err) => {
                    if err == BuckyErrorCode::Interrupted {
                        DownloadTaskState {
                            task_status: TaskStatus::Stopped,
                            err_code: None,
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 0,
                            sum_size: self.chunk_id.len() as u64,
                        }
                    } else {
                        *self.task_status.lock().unwrap() = TaskStatus::Failed;
                        self.task_store
                            .as_ref()
                            .unwrap()
                            .save_task_status(&self.task_id, TaskStatus::Failed)
                            .await?;
                        DownloadTaskState {
                            task_status: TaskStatus::Failed,
                            err_code: Some(err),
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 0,
                            sum_size: 0,
                        }
                    } 
                }
                cyfs_bdt::DownloadTaskState::Finished => {
                    *self.task_status.lock().unwrap() = TaskStatus::Finished;
                    self.task_store
                        .as_ref()
                        .unwrap()
                        .save_task_status(&self.task_id, TaskStatus::Finished)
                        .await?;
                    DownloadTaskState {
                        task_status: TaskStatus::Finished,
                        err_code: None,
                        speed: 0,
                        upload_speed: 0,
                        downloaded_progress: 100,
                        sum_size: self.chunk_id.len() as u64,
                    }
                }
            }
        } else {
            *self.task_status.lock().unwrap() = TaskStatus::Stopped;
            self.task_store
                .as_ref()
                .unwrap()
                .save_task_status(&self.task_id, TaskStatus::Stopped)
                .await?;
            DownloadTaskState {
                task_status: TaskStatus::Stopped,
                err_code: None,
                speed: 0,
                upload_speed: 0,
                downloaded_progress: 0,
                sum_size: self.chunk_id.len() as u64,
            }
        };
        Ok(task_state.to_vec()?)
    }
}

#[derive(RawEncode, RawDecode)]
pub struct DownloadChunkParamV1 {
    pub chunk_id: ChunkId,
    pub device_list: Vec<DeviceId>,
    pub referer: String,
    pub save_path: Option<String>,
    pub context_id: Option<ObjectId>,
}

#[derive(RawEncode, RawDecode)]
pub enum DownloadChunkParam {
    V1(DownloadChunkParamV1),
}

impl DownloadChunkParam {
    pub fn chunk_id(&self) -> &ChunkId {
        match self {
            DownloadChunkParam::V1(param) => &param.chunk_id,
        }
    }

    pub fn device_list(&self) -> &Vec<DeviceId> {
        match self {
            DownloadChunkParam::V1(param) => &param.device_list,
        }
    }

    pub fn referer(&self) -> &str {
        match self {
            DownloadChunkParam::V1(param) => param.referer.as_str(),
        }
    }

    pub fn save_path(&self) -> &Option<String> {
        match self {
            DownloadChunkParam::V1(param) => &param.save_path,
        }
    }

    pub fn context_id(&self) -> &Option<ObjectId> {
        match self {
            DownloadChunkParam::V1(param) => &param.context_id,
        }
    }
}

pub struct DownloadChunkTaskFactory {
    stack: StackGuard,
    chunk_manager: Arc<ChunkManager>,
    trans_store: Arc<TransStore>,
}

impl DownloadChunkTaskFactory {
    pub fn new(
        stack: StackGuard,
        chunk_manager: Arc<ChunkManager>,
        trans_store: Arc<TransStore>,
    ) -> Self {
        Self {
            stack,
            chunk_manager,
            trans_store,
        }
    }
}

#[async_trait::async_trait]
impl TaskFactory for DownloadChunkTaskFactory {
    fn get_task_type(&self) -> TaskType {
        DOWNLOAD_CHUNK_TASK
    }

    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>> {
        let param = DownloadChunkParam::clone_from_slice(params)?;
        let (writer, label_data) =
            if param.save_path().is_some() && !param.save_path().as_ref().unwrap().is_empty() {
                let chunk_writer: Box<dyn ChunkWriter> = Box::new(LocalChunkWriter::new(
                    PathBuf::from(param.save_path().as_ref().unwrap().clone()),
                    self.stack.ndn().chunk_manager().ndc().clone(),
                    self.stack.ndn().chunk_manager().tracker().clone(),
                ));
                (
                    chunk_writer,
                    param.save_path().as_ref().unwrap().as_bytes().to_vec(),
                )
            } else {
                let chunk_writer: Box<dyn ChunkWriter> = Box::new(ChunkManagerWriter::new(
                    self.chunk_manager.clone(),
                    self.stack.ndn().chunk_manager().ndc().clone(),
                    self.stack.ndn().chunk_manager().tracker().clone(),
                ));
                (chunk_writer, Vec::new())
            };

        let task = DownloadChunkTask::new(
            param.chunk_id().clone(),
            self.stack.clone(),
            param.device_list().clone(),
            param.referer().to_string(),
            param.context_id().clone(),
            label_data,
            writer,
        );
        Ok(Box::new(task))
    }

    async fn restore(
        &self,
        _task_status: TaskStatus,
        params: &[u8],
        _data: &[u8],
    ) -> BuckyResult<Box<dyn Task>> {
        let param = DownloadChunkParam::clone_from_slice(params)?;
        let (writer, label_data) =
            if param.save_path().is_some() && !param.save_path().as_ref().unwrap().is_empty() {
                let chunk_writer: Box<dyn ChunkWriter> = Box::new(LocalChunkWriter::new(
                    PathBuf::from(param.save_path().as_ref().unwrap().clone()),
                    self.stack.ndn().chunk_manager().ndc().clone(),
                    self.stack.ndn().chunk_manager().tracker().clone(),
                ));
                (
                    chunk_writer,
                    param.save_path().as_ref().unwrap().as_bytes().to_vec(),
                )
            } else {
                let chunk_writer: Box<dyn ChunkWriter> = Box::new(ChunkManagerWriter::new(
                    self.chunk_manager.clone(),
                    self.stack.ndn().chunk_manager().ndc().clone(),
                    self.stack.ndn().chunk_manager().tracker().clone(),
                ));
                (chunk_writer, Vec::new())
            };

        let task = DownloadChunkTask::new(
            param.chunk_id().clone(),
            self.stack.clone(),
            param.device_list().clone(),
            param.referer().to_string(),
            param.context_id().clone(),
            label_data,
            writer,
        );
        Ok(Box::new(task))
    }
}
