use super::super::download_task_manager::DownloadTaskState;
use crate::ndn_api::{
    ChunkListReaderAdapter, ChunkWriter, ChunkWriterRef, ContextManager,
    LocalChunkWriter,
};
use crate::trans_api::TransStore;
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::{self, StackGuard};
use cyfs_task_manager::*;

use cyfs_debug::Mutex;
use sha2::Digest;
use std::path::PathBuf;
use std::sync::Arc;

pub struct DownloadChunkTask {
    task_id: TaskId,
    chunk_id: ChunkId,
    bdt_stack: StackGuard,
    context_manager: ContextManager,
    dec_id: ObjectId,
    device_list: Vec<DeviceId>,
    referer: String,
    group: Option<String>,
    context: Option<String>,
    session: async_std::sync::Mutex<Option<String>>,
    writer: ChunkWriterRef,
    task_store: Option<Arc<dyn TaskStore>>,
    task_status: Mutex<TaskStatus>,
}

impl DownloadChunkTask {
    pub(crate) fn new(
        chunk_id: ChunkId,
        bdt_stack: StackGuard,
        context_manager: ContextManager,
        dec_id: ObjectId,
        device_list: Vec<DeviceId>,
        referer: String,
        group: Option<String>,
        context: Option<String>,
        task_label_data: Vec<u8>,
        writer: Box<dyn ChunkWriter>,
    ) -> Self {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(DOWNLOAD_CHUNK_TASK.0.to_le_bytes());
        sha256.input(chunk_id.as_slice());
        sha256.input(task_label_data.as_slice());
        let task_id = sha256.result().into();

        Self {
            task_id,
            chunk_id,
            bdt_stack,
            context_manager,
            dec_id,
            device_list,
            referer,
            group,
            context,
            session: async_std::sync::Mutex::new(None),
            writer: Arc::new(writer),
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

        let context = match &self.context {
            Some(context) => {
                self.context_manager
                    .create_download_context_from_trans_context(&self.dec_id, self.referer.clone(), context)
                    .await?
            }
            None => {
                if self.device_list.is_empty() {
                    let msg = format!("invalid chunk task's device list! task={}", self.task_id);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }

                self.context_manager
                    .create_download_context_from_target(self.referer.clone(), self.device_list[0].clone())
                    .await?
            }
        };

        // 创建bdt层的传输任务
        let (id, reader) = cyfs_bdt::download_chunk(
            &self.bdt_stack,
            self.chunk_id.clone(),
            self.group.clone(),
            context,
        )
        .await
        .map_err(|e| {
            error!(
                "start bdt chunk trans session error! task_id={}, {}",
                self.task_id.to_string(),
                e
            );
            e
        })?;

        *session = Some(id);

        ChunkListReaderAdapter::new_chunk(self.writer.clone(), reader, &self.chunk_id).async_run();

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
        let task_group = self.session.lock().await.clone();
        if let Some(id) = task_group {
            let task = self
                .bdt_stack
                .ndn()
                .root_task()
                .download()
                .sub_task(&id)
                .ok_or_else(|| {
                    let msg = format!("get task but ot found! task={}, group={}", self.task_id, id);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::NotFound, msg)
                })?;

            task.pause().map_err(|e| {
                error!(
                    "pause task failed! task={}, group={}, {}",
                    self.task_id, id, e
                );
                e
            })?;
        } else {
            let msg = format!(
                "pause task but task group not exists! task={}",
                self.task_id
            );
            error!("{}", msg);
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
        let task_group = self.session.lock().await.take();
        if let Some(id) = task_group {
            let task = self
                .bdt_stack
                .ndn()
                .root_task()
                .download()
                .sub_task(&id)
                .ok_or_else(|| {
                    let msg = format!("get task but ot found! task={}, group={}", self.task_id, id);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::NotFound, msg)
                })?;

            task.cancel().map_err(|e| {
                error!(
                    "stop task failed! task={}, group={}, {}",
                    self.task_id, id, e
                );
                e
            })?;
        } else {
            let msg = format!("stop task but task group not exists! task={}", self.task_id);
            error!("{}", msg);
        }

        *self.task_status.lock().unwrap() = TaskStatus::Stopped;
        self.task_store
            .as_ref()
            .unwrap()
            .save_task_status(&self.task_id, TaskStatus::Stopped)
            .await?;
        Ok(())
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        let group = self.session.lock().await.clone();
        let task_state = if let Some(id) = &group {
            let task = self
                .bdt_stack
                .ndn()
                .root_task()
                .download()
                .sub_task(&id)
                .ok_or_else(|| {
                    let msg = format!("get task but ot found! task={}, group={}", self.task_id, id);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::NotFound, msg)
                })?;

            let state = task.state();
            match state {

                cyfs_bdt::NdnTaskState::Running => DownloadTaskState {
                    task_status: TaskStatus::Running,
                    err_code: None,
                    speed: task.cur_speed() as u64,
                    upload_speed: 0,
                    downloaded_progress: ((task.downloaded() as f32 / self.chunk_id.len() as f32) * 100.0) as u64,
                    sum_size: self.chunk_id.len() as u64,
                    group,
                },
                cyfs_bdt::NdnTaskState::Paused => DownloadTaskState {
                    task_status: TaskStatus::Paused,
                    err_code: None,
                    speed: 0,
                    upload_speed: 0,
                    downloaded_progress: 0,
                    sum_size: self.chunk_id.len() as u64,
                    group,
                },
                cyfs_bdt::NdnTaskState::Error(err) => {
                    if err.code() == BuckyErrorCode::Interrupted {
                        DownloadTaskState {
                            task_status: TaskStatus::Stopped,
                            err_code: None,
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 0,
                            sum_size: self.chunk_id.len() as u64,
                            group,
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
                            err_code: Some(err.code()),
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 0,
                            sum_size: 0,
                            group,
                        }
                    }
                }
                cyfs_bdt::NdnTaskState::Finished => {
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
                        group,
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
                group: None,
            }
        };
        Ok(task_state.to_vec()?)
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::super::trans_proto::DownloadChunkParam)]
pub struct DownloadChunkParam {
    pub dec_id: ObjectId,
    pub chunk_id: ChunkId,
    pub device_list: Vec<DeviceId>,
    pub referer: String,
    pub save_path: Option<String>,
    pub group: Option<String>,
    pub context: Option<String>,
}

impl ProtobufTransform<super::super::trans_proto::DownloadChunkParam> for DownloadChunkParam {
    fn transform(
        value: crate::trans_api::local::trans_proto::DownloadChunkParam,
    ) -> BuckyResult<Self> {
        let mut device_list = Vec::new();
        for item in value.device_list.iter() {
            device_list.push(DeviceId::clone_from_slice(item.as_slice())?);
        }
        Ok(Self {
            dec_id: ObjectId::clone_from_slice(&value.dec_id)?,
            chunk_id: ChunkId::from(value.chunk_id),
            device_list,
            referer: value.referer,
            save_path: value.save_path,
            context: value.context,
            group: value.group,
        })
    }
}

impl ProtobufTransform<&DownloadChunkParam> for super::super::trans_proto::DownloadChunkParam {
    fn transform(value: &DownloadChunkParam) -> BuckyResult<Self> {
        let mut device_list = Vec::new();
        for item in value.device_list.iter() {
            device_list.push(item.to_vec()?);
        }
        Ok(Self {
            dec_id: value.dec_id.to_vec()?,
            chunk_id: value.chunk_id.as_slice().to_vec(),
            device_list,
            referer: value.referer.clone(),
            save_path: value.save_path.clone(),
            context: value.context.clone(),
            group: value.group.clone(),
        })
    }
}

pub(crate) struct DownloadChunkTaskFactory {
    stack: StackGuard,
    named_data_components: NamedDataComponents,
    trans_store: Arc<TransStore>,
}

impl DownloadChunkTaskFactory {
    pub fn new(
        stack: StackGuard,
        named_data_components: NamedDataComponents,
        trans_store: Arc<TransStore>,
    ) -> Self {
        Self {
            stack,
            named_data_components,
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
            if param.save_path.is_some() && !param.save_path.as_ref().unwrap().is_empty() {
                let chunk_writer: Box<dyn ChunkWriter> = Box::new(LocalChunkWriter::new(
                    PathBuf::from(param.save_path.as_ref().unwrap().clone()),
                    self.named_data_components.ndc.clone(),
                    self.named_data_components.tracker.clone(),
                ));
                (
                    chunk_writer,
                    param.save_path.as_ref().unwrap().as_bytes().to_vec(),
                )
            } else {
                let chunk_writer = self.named_data_components.new_chunk_writer();
                (chunk_writer, Vec::new())
            };

        let task = DownloadChunkTask::new(
            param.chunk_id,
            self.stack.clone(),
            self.named_data_components.context_manager.clone(),
            param.dec_id,
            param.device_list,
            param.referer,
            param.group,
            param.context,
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
            if param.save_path.is_some() && !param.save_path.as_ref().unwrap().is_empty() {
                let chunk_writer: Box<dyn ChunkWriter> = Box::new(LocalChunkWriter::new(
                    PathBuf::from(param.save_path.as_ref().unwrap().clone()),
                    self.named_data_components.ndc.clone(),
                    self.named_data_components.tracker.clone(),
                ));
                (
                    chunk_writer,
                    param.save_path.as_ref().unwrap().as_bytes().to_vec(),
                )
            } else {
                let chunk_writer = self.named_data_components.new_chunk_writer();
                (chunk_writer, Vec::new())
            };

        let task = DownloadChunkTask::new(
            param.chunk_id,
            self.stack.clone(),
            self.named_data_components.context_manager.clone(),
            param.dec_id,
            param.device_list,
            param.referer,
            param.group,
            param.context,
            label_data,
            writer,
        );
        Ok(Box::new(task))
    }
}
