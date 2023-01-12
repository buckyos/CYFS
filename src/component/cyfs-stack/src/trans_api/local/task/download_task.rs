use super::super::download_task_manager::DownloadTaskState;
use super::chunk_task::DownloadChunkParam;
use super::file_task::DownloadFileParam;
use super::verify_file_task::*;
use crate::ndn_api::{
    ChunkListReaderAdapter, ChunkWriter, LocalChunkWriter, LocalFileWriter, TransContextHolder,
};
use crate::trans_api::{DownloadTaskTracker, TransStore};
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::{self, StackGuard};
use cyfs_task_manager::*;

use cyfs_debug::Mutex;
use sha2::Digest;
use std::path::PathBuf;
use std::sync::Arc;

pub(super) struct DownloadFileTaskParams {
    pub task_type: TaskType,
    pub dec_id: ObjectId,

    // file or chunk
    pub file: Option<File>,
    pub chunk_id: Option<ChunkId>,

    pub device_list: Vec<DeviceId>,
    pub referer: String,
    pub save_path: Option<String>,
    pub group: Option<String>,
    pub context: Option<String>,
}

impl DownloadFileTaskParams {
    pub fn new_file(param: DownloadFileParam) -> Self {
        Self {
            task_type: DOWNLOAD_FILE_TASK,

            dec_id: param.dec_id,
            file: Some(param.file),
            chunk_id: None,
            device_list: param.device_list,
            referer: param.referer,
            save_path: param.save_path,
            group: param.group,
            context: param.context,
        }
    }

    pub fn new_chunk(param: DownloadChunkParam) -> Self {
        Self {
            task_type: DOWNLOAD_CHUNK_TASK,

            dec_id: param.dec_id,
            chunk_id: Some(param.chunk_id),
            file: None,
            device_list: param.device_list,
            referer: param.referer,
            save_path: param.save_path,
            group: param.group,
            context: param.context,
        }
    }

    pub fn task_id(&self) -> TaskId {
        let mut sha256 = sha2::Sha256::new();

        sha256.input(self.dec_id.as_slice());
        sha256.input(self.task_type.0.to_le_bytes());
        if let Some(file) = &self.file {
            sha256.input(file.desc().calculate_id().as_slice());
        }
        if let Some(chunk_id) = &self.chunk_id {
            sha256.input(chunk_id.as_slice());
        }
        if let Some(group) = &self.group {
            sha256.input(group.as_bytes());
        }
        if let Some(context) = &self.context {
            sha256.input(context.as_bytes());
        }

        if let Some(save_path) = &self.save_path {
            sha256.input(save_path.as_bytes());
        }

        sha256.result().into()
    }
    pub fn len(&self) -> u64 {
        if let Some(file) = &self.file {
            file.desc().content().len()
        } else if let Some(chunk_id) = &self.chunk_id {
            chunk_id.len() as u64
        } else {
            unreachable!();
        }
    }
}

pub struct DownloadFileTask {
    task_store: Option<Arc<dyn TaskStore>>,
    named_data_components: NamedDataComponents,
    task_id: TaskId,
    bdt_stack: StackGuard,
    params: DownloadFileTaskParams,
    session: async_std::sync::Mutex<Option<String>>,
    verify_task: async_std::sync::Mutex<Option<RunnableTask<VerifyFileRunnable>>>,
    task_status: Mutex<DownloadFileTaskStatus>,
    trans_store: Arc<TransStore>,
}

impl DownloadFileTask {
    pub fn new(
        named_data_components: NamedDataComponents,
        bdt_stack: StackGuard,
        trans_store: Arc<TransStore>,
        task_status: DownloadFileTaskStatus,
        params: DownloadFileTaskParams,
    ) -> Self {
        let task_id = params.task_id();

        Self {
            task_store: None,
            named_data_components,
            task_id,
            bdt_stack,
            params,
            session: async_std::sync::Mutex::new(None),
            verify_task: async_std::sync::Mutex::new(None),
            task_status: Mutex::new(task_status),
            trans_store,
        }
    }

    async fn save_task_status(&self) -> BuckyResult<()> {
        let (task_status, task_data) = {
            let status = self.task_status.lock().unwrap();
            (status.status, status.state.to_vec()?)
        };
        if self.task_store.is_some() {
            self.task_store
                .as_ref()
                .unwrap()
                .save_task(&self.task_id, task_status, task_data)
                .await?;
        }

        let mut conn = self.trans_store.create_connection().await?;
        conn.set_task_status(&self.task_id, task_status).await?;

        Ok(())
    }

    async fn create_context(&self) -> BuckyResult<TransContextHolder> {
        match &self.params.context {
            Some(context) => {
                self.named_data_components
                    .context_manager
                    .create_download_context_from_trans_context(
                        &self.params.dec_id,
                        self.params.referer.clone(),
                        context,
                    )
                    .await
            }
            None => {
                if self.params.device_list.is_empty() {
                    let msg = format!("invalid file task's device list! task={}", self.task_id);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }

                self.named_data_components
                    .context_manager
                    .create_download_context_from_target(
                        self.params.referer.clone(),
                        self.params.device_list[0].clone(),
                    )
                    .await
            }
        }
    }

    async fn create_writer(&self) -> BuckyResult<Box<dyn ChunkWriter>> {
        let writer = if self.params.save_path.is_some()
            && !self.params.save_path.as_ref().unwrap().is_empty()
        {
            let path = PathBuf::from(self.params.save_path.as_ref().unwrap().clone());
            if let Some(file) = &self.params.file {
                Box::new(
                    LocalFileWriter::new(
                        path,
                        file.clone(),
                        self.named_data_components.ndc.clone(),
                        self.named_data_components.tracker.clone(),
                    )
                    .await?,
                ) as Box<dyn ChunkWriter>
            } else if let Some(chunk_id) = &self.params.chunk_id {
                Box::new(LocalChunkWriter::new(
                    path,
                    self.named_data_components.ndc.clone(),
                    self.named_data_components.tracker.clone(),
                )) as Box<dyn ChunkWriter>
            } else {
                unreachable!();
            }
        } else {
            self.named_data_components.new_chunk_writer()
        };

        Ok(writer)
    }
}

#[async_trait::async_trait]
impl Task for DownloadFileTask {
    fn get_task_id(&self) -> TaskId {
        self.task_id.clone()
    }

    fn get_task_type(&self) -> TaskType {
        DOWNLOAD_FILE_TASK
    }

    fn get_task_category(&self) -> TaskCategory {
        DOWNLOAD_TASK_CATEGORY
    }

    async fn get_task_status(&self) -> TaskStatus {
        self.task_status.lock().unwrap().status
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
            if self.task_status.lock().unwrap().status == TaskStatus::Running {
                return Ok(());
            }
        }

        let context = self.create_context().await?;
        let writer = self.create_writer().await?;

        // 创建bdt层的传输任务
        let id = if let Some(file) = &self.params.file {
            let (id, reader) = cyfs_bdt::download_file(
                &self.bdt_stack,
                file.clone(),
                self.params.group.clone(),
                context,
            )
            .await
            .map_err(|e| {
                error!(
                    "start bdt file trans session error! task_id={}, {}",
                    self.task_id, e
                );
                e
            })?;

            ChunkListReaderAdapter::new_file(Arc::new(writer), reader, file).async_run();

            info!(
                "create bdt file trans session success: task={}, file={}, device={:?}, session={}",
                self.task_id,
                file.desc().calculate_id(),
                self.params.device_list,
                id,
            );

            id
        } else if let Some(chunk_id) = &self.params.chunk_id {
            let (id, reader) = cyfs_bdt::download_chunk(
                &self.bdt_stack,
                chunk_id.clone(),
                self.params.group.clone(),
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

            ChunkListReaderAdapter::new_chunk(Arc::new(writer), reader, chunk_id).async_run();

            info!(
                "create bdt chunk trans session success: task={}, chunk={}, device={:?}",
                self.task_id.to_string(),
                chunk_id,
                self.params.device_list,
            );

            id
        } else {
            unreachable!();
        };

        *session = Some(id);

        {
            self.task_status.lock().unwrap().status = TaskStatus::Running;
        }
        self.save_task_status().await?;
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

        {
            self.task_status.lock().unwrap().status = TaskStatus::Paused;
        }
        self.save_task_status().await?;

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

        let mut verify_task = self.verify_task.lock().await;
        if verify_task.is_some() {
            verify_task.take().unwrap().stop_task().await?;
        }

        {
            self.task_status.lock().unwrap().status = TaskStatus::Stopped;
        }
        self.save_task_status().await?;

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
                cyfs_bdt::DownloadTaskState::Downloading => {
                    log::info!("downloading speed {}", task.cur_speed());
                    let progress =
                        ((task.downloaded() as f32 / self.params.len() as f32) * 100.0) as u64;
                    {
                        let mut task_status = self.task_status.lock().unwrap();
                        task_status.status = TaskStatus::Running;
                        task_status.state.set_download_progress(progress);
                    }
                    self.save_task_status().await?;
                    DownloadTaskState {
                        task_status: TaskStatus::Running,
                        err_code: None,
                        speed: task.cur_speed() as u64,
                        upload_speed: 0,
                        downloaded_progress: progress,
                        sum_size: self.params.len(),
                        group,
                    }
                }
                cyfs_bdt::DownloadTaskState::Paused => {
                    {
                        let mut status = self.task_status.lock().unwrap();
                        status.status = TaskStatus::Paused;
                    };
                    DownloadTaskState {
                        task_status: TaskStatus::Paused,
                        err_code: None,
                        speed: 0,
                        upload_speed: 0,
                        downloaded_progress: 100,
                        sum_size: self.params.len(),
                        group,
                    }
                }
                cyfs_bdt::DownloadTaskState::Finished => {
                    let mut verify_task = self.verify_task.lock().await;
                    if verify_task.is_some() {
                        let task = verify_task.as_ref().unwrap();
                        let verify_task_status = task.get_task_status().await;
                        if TaskStatus::Running == verify_task_status {
                            self.task_status.lock().unwrap().status = TaskStatus::Running;
                            DownloadTaskState {
                                task_status: TaskStatus::Running,
                                err_code: None,
                                speed: 0,
                                upload_speed: 0,
                                downloaded_progress: 100,
                                sum_size: self.params.len(),
                                group,
                            }
                        } else if TaskStatus::Finished == verify_task_status {
                            let ret = bool::clone_from_slice(
                                task.get_task_detail_status().await?.as_slice(),
                            )?;
                            if ret {
                                self.task_status.lock().unwrap().status = TaskStatus::Finished;
                                self.save_task_status().await?;
                                DownloadTaskState {
                                    task_status: TaskStatus::Finished,
                                    err_code: None,
                                    speed: 0,
                                    upload_speed: 0,
                                    downloaded_progress: 100,
                                    sum_size: self.params.len(),
                                    group,
                                }
                            } else {
                                self.task_status.lock().unwrap().status = TaskStatus::Failed;
                                self.save_task_status().await?;
                                DownloadTaskState {
                                    task_status: TaskStatus::Failed,
                                    err_code: Some(BuckyErrorCode::InvalidData),
                                    speed: 0,
                                    upload_speed: 0,
                                    downloaded_progress: 100,
                                    sum_size: self.params.len(),
                                    group,
                                }
                            }
                        } else {
                            self.task_status.lock().unwrap().status = TaskStatus::Stopped;
                            self.save_task_status().await?;
                            DownloadTaskState {
                                task_status: TaskStatus::Stopped,
                                err_code: Some(BuckyErrorCode::InvalidData),
                                speed: 0,
                                upload_speed: 0,
                                downloaded_progress: 100,
                                sum_size: self.params.len(),
                                group,
                            }
                        }
                    } else {
                        let task = RunnableTask::new(VerifyFileRunnable::new(
                            self.named_data_components.chunk_manager.clone(),
                            self.task_id.clone(),
                            self.params.file.clone(),
                            self.params.chunk_id.clone(),
                            self.params.save_path.clone(),
                        ));
                        task.start_task().await?;
                        *verify_task = Some(task);

                        self.task_status.lock().unwrap().status = TaskStatus::Running;
                        DownloadTaskState {
                            task_status: TaskStatus::Running,
                            err_code: None,
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 100,
                            sum_size: self.params.len(),
                            group,
                        }
                    }
                }
                cyfs_bdt::DownloadTaskState::Error(err) => {
                    if err.code() == BuckyErrorCode::Interrupted {
                        {
                            let mut status = self.task_status.lock().unwrap();
                            status.status = TaskStatus::Stopped;
                        };
                        DownloadTaskState {
                            task_status: TaskStatus::Stopped,
                            err_code: None,
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 100,
                            sum_size: self.params.len(),
                            group,
                        }
                    } else {
                        self.task_status.lock().unwrap().status = TaskStatus::Failed;
                        self.save_task_status().await?;
                        DownloadTaskState {
                            task_status: TaskStatus::Failed,
                            err_code: Some(err.code()),
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 0,
                            sum_size: self.params.len(),
                            group,
                        }
                    }
                }
            }
        } else {
            self.task_status.lock().unwrap().status = TaskStatus::Stopped;
            DownloadTaskState {
                task_status: TaskStatus::Stopped,
                err_code: None,
                speed: 0,
                upload_speed: 0,
                downloaded_progress: 0,
                sum_size: self.params.len(),
                group: None,
            }
        };
        Ok(task_state.to_vec()?)
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(super::super::trans_proto::DownloadFileTaskState)]
pub(super) struct DownloadFileTaskState {
    download_progress: u64,
}

impl DownloadFileTaskState {
    pub fn new(download_progress: u64) -> Self {
        DownloadFileTaskState { download_progress }
    }

    pub fn download_progress(&self) -> u64 {
        self.download_progress
    }

    pub fn set_download_progress(&mut self, download_progress: u64) {
        if download_progress > self.download_progress {
            self.download_progress = download_progress;
        }
    }
}

pub(super) struct DownloadFileTaskStatus {
    status: TaskStatus,
    state: DownloadFileTaskState,
}

impl DownloadFileTaskStatus {
    pub fn new() -> Self {
        Self {
            status: TaskStatus::Stopped,
            state: DownloadFileTaskState::new(0),
        }
    }

    pub fn load(data: &[u8]) -> BuckyResult<Self> {
        let ret = if data.len() > 0 {
            Self {
                status: TaskStatus::Stopped,
                state: DownloadFileTaskState::clone_from_slice(data)?,
            }
        } else {
            Self::new()
        };

        Ok(ret)
    }
}