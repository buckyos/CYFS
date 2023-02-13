use super::super::download_task_manager::DownloadTaskState;
use super::chunk_task::DownloadChunkParam;
use super::file_task::DownloadFileParam;
use super::verify_file_task::*;
use crate::trans_api::{DownloadTaskTracker, TransStore};
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::{self, LeafDownloadTask, StackGuard};
use cyfs_bdt_ext::{
    ChunkListReaderAdapter, ChunkWriter, LocalChunkWriter, LocalFileWriter, NDNTaskCancelStrategy,
    TransContextHolder,
};
use cyfs_task_manager::*;

use async_std::sync::Mutex as AsyncMutex;
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

pub(super) struct DownloadFileTask {
    task_store: Option<Arc<dyn TaskStore>>,
    named_data_components: NamedDataComponents,
    task_id: TaskId,
    bdt_stack: StackGuard,
    params: DownloadFileTaskParams,
    session: async_std::sync::Mutex<Option<Box<dyn LeafDownloadTask>>>,
    verify_task: async_std::sync::Mutex<Option<RunnableTask<VerifyFileRunnable>>>,
    task_status: AsyncMutex<DownloadFileTaskStatus>,
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
            task_status: AsyncMutex::new(task_status),
            trans_store,
        }
    }

    async fn get_session(&self) -> Option<Box<dyn LeafDownloadTask>> {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|v| v.clone_as_leaf_task())
    }

    async fn take_session(&self) -> Option<Box<dyn LeafDownloadTask>> {
        self.session.lock().await.take()
    }

    async fn save_task_status(&self, status: &DownloadFileTaskStatus) -> BuckyResult<()> {
        let task_status = status.status;
        let task_data = status.state.to_vec()?;

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
                        NDNTaskCancelStrategy::WaitingSource,
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
            let path = self.params.save_path.as_ref().unwrap();

            #[cfg(not(windows))]
            let path = path.replace("\\", "/");

            let path = PathBuf::from(path.to_owned());
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
                    chunk_id.to_owned(),
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

    async fn create_task(&self) -> BuckyResult<(String, Box<dyn LeafDownloadTask>)> {
        let context = self.create_context().await?;
        let writer = self.create_writer().await?;

        // 创建bdt层的传输任务
        let ret = if let Some(file) = &self.params.file {
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

            let task = reader.task().clone_as_leaf_task();
            ChunkListReaderAdapter::new_file(Arc::new(writer), reader, file).async_run();

            info!(
                "create bdt file trans session success: task={}, file={}, device={:?}, session={}",
                self.task_id,
                file.desc().calculate_id(),
                self.params.device_list,
                id,
            );

            (id, task)
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

            let task = reader.task().clone_as_leaf_task();
            ChunkListReaderAdapter::new_chunk(Arc::new(writer), reader, chunk_id).async_run();

            info!(
                "create bdt chunk trans session success: task={}, chunk={}, device={:?}",
                self.task_id.to_string(),
                chunk_id,
                self.params.device_list,
            );

            (id, task)
        } else {
            unreachable!();
        };

        Ok(ret)
    }

    async fn start_verify(&self) -> BuckyResult<RunnableTask<VerifyFileRunnable>> {
        info!(
            "will start verify task for download task! task={}",
            self.task_id
        );

        let vtask = RunnableTask::new(VerifyFileRunnable::new(
            self.named_data_components.chunk_manager.clone(),
            self.task_id.clone(),
            self.params.file.clone(),
            self.params.chunk_id.clone(),
            self.params.save_path.clone(),
        ));
        vtask.start_task().await?;

        Ok(vtask)
    }

    async fn get_task_status_with_verify_task(
        &self,
        verify_task: &RunnableTask<VerifyFileRunnable>,
    ) -> BuckyResult<DownloadTaskState> {
        let verify_task_status = verify_task.get_task_status().await;

        let state = match verify_task_status {
            TaskStatus::Running | TaskStatus::Paused => DownloadTaskState {
                task_status: TaskStatus::Running,
                err_code: None,
                speed: 0,
                upload_speed: 0,
                downloaded_progress: 100,
                sum_size: self.params.len(),
                group: self.params.group.clone(),
            },
            TaskStatus::Finished => {
                let ret =
                    bool::clone_from_slice(verify_task.get_task_detail_status().await?.as_slice())?;

                if ret {
                    DownloadTaskState {
                        task_status: TaskStatus::Finished,
                        err_code: None,
                        speed: 0,
                        upload_speed: 0,
                        downloaded_progress: 100,
                        sum_size: self.params.len(),
                        group: self.params.group.clone(),
                    }
                } else {
                    let msg = format!(
                        "verify download task but got invalid data! task={}",
                        self.task_id
                    );
                    error!("{}", msg);

                    DownloadTaskState {
                        task_status: TaskStatus::Failed,
                        err_code: Some(BuckyErrorCode::InvalidData),
                        speed: 0,
                        upload_speed: 0,
                        downloaded_progress: 100,
                        sum_size: self.params.len(),
                        group: self.params.group.clone(),
                    }
                }
            }
            TaskStatus::Failed => {
                let msg = format!("verify download task but got error! task={}", self.task_id);
                error!("{}", msg);

                DownloadTaskState {
                    task_status: TaskStatus::Failed,
                    err_code: Some(BuckyErrorCode::InvalidData),
                    speed: 0,
                    upload_speed: 0,
                    downloaded_progress: 100,
                    sum_size: self.params.len(),
                    group: self.params.group.clone(),
                }
            }
            TaskStatus::Stopped => {
                // should not come here?
                let msg = format!(
                    "verify download task but got error state! task={}",
                    self.task_id
                );
                error!("{}", msg);

                DownloadTaskState {
                    task_status: TaskStatus::Stopped,
                    err_code: Some(BuckyErrorCode::InvalidData),
                    speed: 0,
                    upload_speed: 0,
                    downloaded_progress: 100,
                    sum_size: self.params.len(),
                    group: self.params.group.clone(),
                }
            }
        };

        Ok(state)
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
        self.task_status.lock().await.status
    }

    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>) {
        self.task_store = Some(task_store);
    }

    async fn start_task(&self) -> BuckyResult<()> {
        let mut task_status = self.task_status.lock().await;
        if task_status.status == TaskStatus::Running {
            warn!(
                "start download task but already in running! task={}",
                self.task_id
            );
            return Ok(());
        }

        let (id, task) = self.create_task().await?;

        info!("start download task: task={}, group={}", self.task_id, id);
        {
            let mut session = self.session.lock().await;
            assert!(session.is_none());
            *session = Some(task);
        }

        task_status.status = TaskStatus::Running;

        self.save_task_status(&task_status).await?;
        Ok(())
    }

    async fn pause_task(&self) -> BuckyResult<()> {
        let mut task_status = self.task_status.lock().await;
        match task_status.status {
            TaskStatus::Paused => {
                warn!(
                    "pause download task but already been paused! task={}",
                    self.task_id
                );
                return Ok(());
            }
            TaskStatus::Running => {}
            status @ _ => {
                let msg = format!(
                    "pause download task but not in running! task={}, state={:?}",
                    self.task_id, status,
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
            }
        }

        let session = self.get_session().await;
        if let Some(session) = session {
            session.pause().map_err(|e| {
                error!(
                    "pause task failed! task={}, group={:?}, {}",
                    self.task_id,
                    session.abs_group_path(),
                    e
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

        task_status.status = TaskStatus::Paused;

        self.save_task_status(&task_status).await?;

        Ok(())
    }

    async fn stop_task(&self) -> BuckyResult<()> {
        let mut task_status = self.task_status.lock().await;
        match task_status.status {
            TaskStatus::Paused | TaskStatus::Running => {}
            status @ _ => {
                let msg = format!(
                    "stop download task but not in running or pause state! task={}, state={:?}",
                    self.task_id, status,
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
            }
        }

        let session = self.take_session().await;
        if let Some(session) = session {
            session.cancel().map_err(|e| {
                error!(
                    "stop task failed! task={}, group={:?}, {}",
                    self.task_id,
                    session.abs_group_path(),
                    e
                );
                e
            })?;
        } else {
            let msg = format!(
                "stop task but task session not exists! task={}",
                self.task_id
            );
            error!("{}", msg);
        }

        let mut verify_task = self.verify_task.lock().await;
        if verify_task.is_some() {
            info!("will stop download verify task! task={}", self.task_id);
            if let Err(e) = verify_task.take().unwrap().stop_task().await {
                error!(
                    "stop download verify task failed! task={}, {}",
                    self.task_id, e
                );
            }
        }

        task_status.status = TaskStatus::Stopped;

        self.save_task_status(&task_status).await?;

        Ok(())
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        let mut task_status = self.task_status.lock().await;

        let session = self.get_session().await;
        let ret = if let Some(session) = session {
            let len = self.params.len();
            let progress = if len > 0 {
                ((session.transfered() as f32 / len as f32) * 100.0) as u64
            } else {
                100
            };

            let state = session.state();
            match state {
                cyfs_bdt::NdnTaskState::Running => {
                    let speed = session.cur_speed();
                    debug!(
                        "task in downloading: task={}, speed={}",
                        self.task_id, speed
                    );

                    if task_status.status != TaskStatus::Running {
                        warn!("download task state not matched! task={}, session state={:?}, task state={:?}", 
                            self.task_id, state, task_status.status);
                    }

                    DownloadTaskState {
                        task_status: TaskStatus::Running,
                        err_code: None,
                        speed: speed as u64,
                        upload_speed: 0,
                        downloaded_progress: progress,
                        sum_size: len,
                        group: self.params.group.clone(),
                    }
                }
                cyfs_bdt::NdnTaskState::Paused => {
                    if task_status.status != TaskStatus::Paused {
                        warn!("download task state not matched! task={}, session state={:?}, task state={:?}", self.task_id, state, task_status.status);
                    }

                    DownloadTaskState {
                        task_status: TaskStatus::Paused,
                        err_code: None,
                        speed: 0,
                        upload_speed: 0,
                        downloaded_progress: progress,
                        sum_size: len,
                        group: self.params.group.clone(),
                    }
                }
                cyfs_bdt::NdnTaskState::Finished => {
                    info!(
                        "download task bdt session finished! task={}, path={:?}",
                        self.task_id,
                        session.abs_group_path()
                    );

                    // should close the bdt session!
                    self.take_session().await;

                    // try start the verify task!
                    if task_status.status == TaskStatus::Running
                        || task_status.status == TaskStatus::Paused
                    {
                        let mut verify_task = self.verify_task.lock().await;
                        assert!(verify_task.is_none());

                        *verify_task = Some(self.start_verify().await?);

                        DownloadTaskState {
                            task_status: TaskStatus::Running,
                            err_code: None,
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 100,
                            sum_size: len,
                            group: self.params.group.clone(),
                        }
                    } else {
                        error!("download session finished but task state is not running or paused! task={}, task state={:?}", self.task_id, task_status.status);
                        DownloadTaskState {
                            task_status: task_status.status,
                            err_code: None,
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 100,
                            sum_size: len,
                            group: self.params.group.clone(),
                        }
                    }
                }
                cyfs_bdt::NdnTaskState::Error(err) => {
                    self.take_session().await;

                    if err.code() == BuckyErrorCode::Interrupted {
                        DownloadTaskState {
                            task_status: TaskStatus::Stopped,
                            err_code: None,
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 100,
                            sum_size: len,
                            group: self.params.group.clone(),
                        }
                    } else {
                        DownloadTaskState {
                            task_status: TaskStatus::Failed,
                            err_code: Some(err.code()),
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 0,
                            sum_size: len,
                            group: self.params.group.clone(),
                        }
                    }
                }
            }
        } else {
            let mut verify_task = self.verify_task.lock().await;
            if let Some(task) = &*verify_task {
                let state = self.get_task_status_with_verify_task(task).await?;

                if state.task_status != TaskStatus::Running
                    && state.task_status != TaskStatus::Paused
                {
                    *verify_task = None;
                }

                state
            } else {
                let status = if task_status.status == TaskStatus::Paused
                    || task_status.status == TaskStatus::Running
                {
                    error!("download task state not matched! task={}, session is empty, but task state={:?}", self.task_id, task_status.status);
                    TaskStatus::Stopped
                } else {
                    task_status.status
                };

                DownloadTaskState {
                    task_status: status,
                    err_code: None,
                    speed: 0,
                    upload_speed: 0,
                    downloaded_progress: task_status.state.download_progress,
                    sum_size: self.params.len(),
                    group: self.params.group.clone(),
                }
            }
        };

        let mut changed = false;
        if task_status.status != ret.task_status {
            info!(
                "download task status updated! {:?} -> {:?}",
                task_status.status, ret.task_status
            );
            changed = true;
            task_status.status = ret.task_status;
        }
        if task_status.state.download_progress != ret.downloaded_progress {
            info!(
                "download task progress updated! {} -> {}",
                task_status.state.download_progress, ret.downloaded_progress
            );
            changed = true;
            task_status.state.download_progress = ret.downloaded_progress;
        }

        if changed {
            self.save_task_status(&task_status).await?;
        }

        Ok(ret.to_vec()?)
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
