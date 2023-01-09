use super::super::download_task_manager::DownloadTaskState;
use super::verify_file_task::*;
use crate::ndn_api::{ChunkListReaderAdapter, ChunkWriter, LocalFileWriter};
use crate::trans_api::{DownloadTaskTracker, TransStore};
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::{self, StackGuard};
use cyfs_task_manager::*;

use cyfs_debug::Mutex;
use sha2::Digest;
use std::path::PathBuf;
use std::sync::Arc;

struct DownloadFileTaskStatus {
    status: TaskStatus,
    state: DownloadFileTaskState,
}
pub struct DownloadFileTask {
    task_store: Option<Arc<dyn TaskStore>>,
    named_data_components: NamedDataComponents,
    task_id: TaskId,
    bdt_stack: StackGuard,
    dec_id: ObjectId,
    device_list: Vec<DeviceId>,
    referer: String,
    file: File,
    save_path: Option<String>,
    group: Option<String>,
    context: Option<String>,
    session: async_std::sync::Mutex<Option<String>>,
    verify_task: async_std::sync::Mutex<Option<RunnableTask<VerifyFileRunnable>>>,
    task_status: Mutex<DownloadFileTaskStatus>,
    trans_store: Arc<TransStore>,
}

impl DownloadFileTask {
    fn new(
        named_data_components: NamedDataComponents,
        bdt_stack: StackGuard,
        dec_id: ObjectId,
        device_list: Vec<DeviceId>,
        referer: String,
        file: File,
        save_path: Option<String>,
        group: Option<String>,
        context: Option<String>,
        trans_store: Arc<TransStore>,
        task_status: DownloadFileTaskStatus,
    ) -> Self {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(DOWNLOAD_FILE_TASK.0.to_le_bytes());
        sha256.input(file.desc().calculate_id().as_slice());
        if save_path.is_some() {
            sha256.input(save_path.as_ref().unwrap().as_bytes());
        }
        let task_id = sha256.result().into();

        Self {
            task_store: None,
            named_data_components,
            task_id,
            bdt_stack,
            dec_id,
            device_list,
            referer,
            file,
            save_path,
            group,
            context,
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

        let context = match &self.context {
            Some(context) => {
                self.named_data_components
                    .context_manager
                    .create_download_context_from_trans_context(&self.dec_id, self.referer.clone(), context)
                    .await?
            }
            None => {
                if self.device_list.is_empty() {
                    let msg = format!("invalid file task's device list! task={}", self.task_id);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }

                self.named_data_components
                    .context_manager
                    .create_download_context_from_target(self.referer.clone(), self.device_list[0].clone())
                    .await?
            }
        };

        let writer: Box<dyn ChunkWriter> =
            if self.save_path.is_some() && !self.save_path.as_ref().unwrap().is_empty() {
                Box::new(
                    LocalFileWriter::new(
                        PathBuf::from(self.save_path.as_ref().unwrap().clone()),
                        self.file.clone(),
                        self.named_data_components.ndc.clone(),
                        self.named_data_components.tracker.clone(),
                    )
                    .await?,
                )
            } else {
                self.named_data_components.new_chunk_writer()
            };

        // 创建bdt层的传输任务
        let (id, reader) = cyfs_bdt::download_file(
            &self.bdt_stack,
            self.file.clone(),
            self.group.clone(),
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

        *session = Some(id);

        ChunkListReaderAdapter::new_file(Arc::new(writer), reader, &self.file).async_run();

        info!(
            "create bdt file trans session success: task={}, device={:?}",
            self.task_id, self.device_list,
        );

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
                cyfs_bdt::NdnTaskState::Downloading => {
                    log::info!("downloading speed {}", task.cur_speed());
                    let progress = ((task.downloaded() as f32 / self.file.desc().content().len() as f32) * 100.0) as u64;
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
                        sum_size: self.file.desc().content().len() as u64,
                        group,
                    }
                }
                cyfs_bdt::NdnTaskState::Paused => {
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
                        sum_size: self.file.desc().content().len() as u64,
                        group,
                    }
                }
                cyfs_bdt::NdnTaskState::Finished => {
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
                                sum_size: self.file.desc().content().len() as u64,
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
                                    sum_size: self.file.desc().content().len() as u64,
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
                                    sum_size: self.file.desc().content().len() as u64,
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
                                sum_size: self.file.desc().content().len() as u64,
                                group,
                            }
                        }
                    } else {
                        let task = RunnableTask::new(VerifyFileRunnable::new(
                            self.named_data_components.chunk_manager.clone(),
                            self.task_id.clone(),
                            self.file.clone(),
                            self.save_path.clone(),
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
                            sum_size: self.file.desc().content().len() as u64,
                            group,
                        }
                    }
                }
                cyfs_bdt::NdnTaskState::Error(err) => {
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
                            sum_size: self.file.desc().content().len() as u64,
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
                            sum_size: self.file.desc().content().len() as u64,
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
                sum_size: self.file.desc().content().len() as u64,
                group: None,
            }
        };
        Ok(task_state.to_vec()?)
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::super::trans_proto::DownloadFileParam)]
pub struct DownloadFileParam {
    pub dec_id: ObjectId,
    pub file: File,
    pub device_list: Vec<DeviceId>,
    pub referer: String,
    pub save_path: Option<String>,
    pub group: Option<String>,
    pub context: Option<String>,
}

impl ProtobufTransform<super::super::trans_proto::DownloadFileParam> for DownloadFileParam {
    fn transform(
        value: crate::trans_api::local::trans_proto::DownloadFileParam,
    ) -> BuckyResult<Self> {
        let mut device_list = Vec::new();
        for item in value.device_list.iter() {
            device_list.push(DeviceId::clone_from_slice(item.as_slice())?);
        }
        Ok(Self {
            dec_id: ObjectId::clone_from_slice(&value.dec_id)?,
            file: File::clone_from_slice(value.file.as_slice())?,
            device_list,
            referer: value.referer,
            save_path: value.save_path,
            context: value.context,
            group: value.group,
        })
    }
}

impl ProtobufTransform<&DownloadFileParam> for super::super::trans_proto::DownloadFileParam {
    fn transform(value: &DownloadFileParam) -> BuckyResult<Self> {
        let mut device_list = Vec::new();
        for item in value.device_list.iter() {
            device_list.push(item.to_vec()?);
        }
        Ok(Self {
            dec_id: value.dec_id.to_vec()?,
            file: value.file.to_vec()?,
            device_list,
            referer: value.referer.clone(),
            save_path: value.save_path.clone(),
            context: value.context.clone(),
            group: value.group.clone(),
        })
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(super::super::trans_proto::DownloadFileTaskState)]
struct DownloadFileTaskState {
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

pub(crate) struct DownloadFileTaskFactory {
    named_data_components: NamedDataComponents,
    stack: StackGuard,
    trans_store: Arc<TransStore>,
}

impl DownloadFileTaskFactory {
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
impl TaskFactory for DownloadFileTaskFactory {
    fn get_task_type(&self) -> TaskType {
        DOWNLOAD_FILE_TASK
    }

    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>> {
        let params = DownloadFileParam::clone_from_slice(params)?;
        let task = DownloadFileTask::new(
            self.named_data_components.clone(),
            self.stack.clone(),
            params.dec_id,
            params.device_list,
            params.referer,
            params.file,
            params.save_path,
            params.group,
            params.context,
            self.trans_store.clone(),
            DownloadFileTaskStatus {
                status: TaskStatus::Stopped,
                state: DownloadFileTaskState::new(0),
            },
        );
        Ok(Box::new(task))
    }

    async fn restore(
        &self,
        _task_status: TaskStatus,
        params: &[u8],
        _data: &[u8],
    ) -> BuckyResult<Box<dyn Task>> {
        let params = DownloadFileParam::clone_from_slice(params)?;
        let data = if _data.len() > 0 {
            DownloadFileTaskStatus {
                status: TaskStatus::Stopped,
                state: DownloadFileTaskState::clone_from_slice(_data)?,
            }
        } else {
            DownloadFileTaskStatus {
                status: TaskStatus::Stopped,
                state: DownloadFileTaskState::new(0),
            }
        };
        let task = DownloadFileTask::new(
            self.named_data_components.clone(),
            self.stack.clone(),
            params.dec_id,
            params.device_list,
            params.referer,
            params.file,
            params.save_path,
            params.group,
            params.context,
            self.trans_store.clone(),
            data,
        );
        Ok(Box::new(task))
    }
}
