use super::super::download_task_manager::DownloadTaskState;
use super::verify_file_task::*;
use crate::ndn_api::{ChunkManagerWriter, LocalFileWriter};
use crate::trans_api::{DownloadTaskTracker, TransStore};
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
use std::sync::Arc;
use cyfs_debug::Mutex;

struct DownloadFileTaskStatus {
    status: TaskStatus,
    state: DownloadFileTaskState,
}
pub struct DownloadFileTask {
    task_store: Option<Arc<dyn TaskStore>>,
    chunk_manager: Arc<ChunkManager>,
    task_id: TaskId,
    bdt_stack: StackGuard,
    device_list: Vec<DeviceId>,
    referer: String,
    file: File,
    save_path: Option<String>,
    context_id: Option<ObjectId>,
    session: async_std::sync::Mutex<Option<Box<dyn cyfs_bdt::DownloadTask>>>,
    verify_task: async_std::sync::Mutex<Option<RunnableTask<VerifyFileRunnable>>>,
    task_status: Mutex<DownloadFileTaskStatus>,
    trans_store: Arc<TransStore>,
}

impl DownloadFileTask {
    fn new(
        chunk_manager: Arc<ChunkManager>,
        bdt_stack: StackGuard,
        device_list: Vec<DeviceId>,
        referer: String,
        file: File,
        save_path: Option<String>,
        context_id: Option<ObjectId>,
        trans_store: Arc<TransStore>,
        task_status: DownloadFileTaskStatus,
    ) -> Self {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(file.desc().calculate_id().as_slice());
        if save_path.is_some() {
            sha256.input(save_path.as_ref().unwrap().as_bytes());
        }
        let task_id = sha256.result().into();

        Self {
            task_store: None,
            chunk_manager,
            task_id,
            bdt_stack,
            device_list,
            referer,
            file,
            save_path,
            context_id,
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
        let context = SingleDownloadContext::streams(
            if self.referer.len() > 0 {
                Some(self.referer.to_owned())
            } else {
                None
            }, self.device_list.clone());
        

        let writers: Box<dyn ChunkWriter> =
            if self.save_path.is_some() && !self.save_path.as_ref().unwrap().is_empty() {
                Box::new(
                    LocalFileWriter::new(
                        PathBuf::from(self.save_path.as_ref().unwrap().clone()),
                        self.file.clone(),
                        self.bdt_stack.ndn().chunk_manager().ndc().clone(),
                        self.bdt_stack.ndn().chunk_manager().tracker().clone(),
                    )
                    .await?,
                )
            } else {
                Box::new(ChunkManagerWriter::new(
                    self.chunk_manager.clone(),
                    self.bdt_stack.ndn().chunk_manager().ndc().clone(),
                    self.bdt_stack.ndn().chunk_manager().tracker().clone(),
                ))
            };

        // 创建bdt层的传输任务
        *session = Some(
            cyfs_bdt::download::download_file(
                &self.bdt_stack,
                self.file.clone(), 
                None, 
                Some(context),
                vec![writers],
            )
            .await
            .map_err(|e| {
                error!(
                    "start bdt file trans session error! task_id={}, {}",
                    self.task_id, e
                );
                e
            })?,
        );

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
        let session = self.session.lock().await;
        if session.is_some() {
            session.as_ref().unwrap().pause()?;
        }

        {
            self.task_status.lock().unwrap().status = TaskStatus::Paused;
        }
        self.save_task_status().await?;

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
        let session = self.session.lock().await;
        let task_state = if session.is_some() {
            let state = session.as_ref().unwrap().state();
            match state {
                cyfs_bdt::DownloadTaskState::Downloading(speed, progress) => {
                    log::info!("downloading speed {} progress {}", speed, progress);
                    {
                        let mut task_status = self.task_status.lock().unwrap();
                        task_status.status = TaskStatus::Running;
                        task_status.state.set_download_progress(progress as u64);
                    }
                    self.save_task_status().await?;
                    DownloadTaskState {
                        task_status: TaskStatus::Running,
                        err_code: None,
                        speed: speed as u64,
                        upload_speed: 0,
                        downloaded_progress: progress as u64,
                        sum_size: self.file.desc().content().len() as u64,
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
                        sum_size: self.file.desc().content().len() as u64,
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
                                sum_size: self.file.desc().content().len() as u64,
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
                            }
                        }
                    } else {
                        let task = RunnableTask::new(VerifyFileRunnable::new(
                            self.chunk_manager.clone(),
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
                        }
                    }
                }
                cyfs_bdt::DownloadTaskState::Error(err) => {
                    if err == BuckyErrorCode::Interrupted {
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
                        }
                    } else {
                        self.task_status.lock().unwrap().status = TaskStatus::Failed;
                        self.save_task_status().await?;
                        DownloadTaskState {
                            task_status: TaskStatus::Failed,
                            err_code: Some(err),
                            speed: 0,
                            upload_speed: 0,
                            downloaded_progress: 0,
                            sum_size: self.file.desc().content().len() as u64,
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
            }
        };
        Ok(task_state.to_vec()?)
    }
}

#[derive(RawEncode, RawDecode)]
pub struct DownloadFileParamV1 {
    pub file: File,
    pub device_list: Vec<DeviceId>,
    pub referer: String,
    pub save_path: Option<String>,
    pub context_id: Option<ObjectId>,
}

#[derive(RawEncode, RawDecode)]
pub enum DownloadFileParam {
    V1(DownloadFileParamV1),
}

#[derive(RawEncode, RawDecode)]
struct DownloadFileTaskStateV1 {
    download_progress: u64,
}

#[derive(RawEncode, RawDecode)]
enum DownloadFileTaskState {
    V1(DownloadFileTaskStateV1),
}

impl DownloadFileTaskState {
    pub fn new(download_progress: u64) -> Self {
        DownloadFileTaskState::V1(DownloadFileTaskStateV1 { download_progress })
    }

    pub fn download_progress(&self) -> u64 {
        match self {
            Self::V1(state) => state.download_progress,
        }
    }

    pub fn set_download_progress(&mut self, download_progress: u64) {
        match self {
            Self::V1(state) => {
                if download_progress > state.download_progress {
                    state.download_progress = download_progress;
                }
            }
        }
    }
}
impl DownloadFileParam {
    pub fn file(&self) -> &File {
        match self {
            DownloadFileParam::V1(param) => &param.file,
        }
    }

    pub fn device_list(&self) -> &Vec<DeviceId> {
        match self {
            DownloadFileParam::V1(param) => &param.device_list,
        }
    }

    pub fn referer(&self) -> &str {
        match self {
            DownloadFileParam::V1(param) => param.referer.as_str(),
        }
    }

    pub fn save_path(&self) -> &Option<String> {
        match self {
            DownloadFileParam::V1(param) => &param.save_path,
        }
    }

    pub fn context_id(&self) -> &Option<ObjectId> {
        match self {
            DownloadFileParam::V1(param) => &param.context_id,
        }
    }
}

pub(crate) struct DownloadFileTaskFactory {
    chunk_manager: Arc<ChunkManager>,
    stack: StackGuard,
    trans_store: Arc<TransStore>,
}

impl DownloadFileTaskFactory {
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
impl TaskFactory for DownloadFileTaskFactory {
    fn get_task_type(&self) -> TaskType {
        DOWNLOAD_FILE_TASK
    }

    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>> {
        let params = DownloadFileParam::clone_from_slice(params)?;
        let task = DownloadFileTask::new(
            self.chunk_manager.clone(),
            self.stack.clone(),
            params.device_list().clone(),
            params.referer().to_string(),
            params.file().clone(),
            params.save_path().clone(),
            params.context_id().clone(),
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
            self.chunk_manager.clone(),
            self.stack.clone(),
            params.device_list().clone(),
            params.referer().to_string(),
            params.file().clone(),
            params.save_path().clone(),
            params.context_id().clone(),
            self.trans_store.clone(),
            data,
        );
        Ok(Box::new(task))
    }
}
