use crate::resolver::OodResolver;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;

use crate::trans::TransInputProcessor;
use crate::trans_api::local::FileRecorder;
use crate::trans_api::{DownloadTaskManager, PublishManager, TransStore};
use cyfs_base::File;
use cyfs_chunk_cache::ChunkManager;
use cyfs_core::{TransContext, TransContextObject};
use cyfs_task_manager::{TaskId, TaskManager, TaskStatus};
use std::convert::TryFrom;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

fn trans_task_status_to_task_status(task_status: TransTaskStatus) -> TaskStatus {
    match task_status {
        TransTaskStatus::Stopped => TaskStatus::Stopped,
        TransTaskStatus::Finished => TaskStatus::Finished,
        TransTaskStatus::Running => TaskStatus::Running,
        TransTaskStatus::Failed => TaskStatus::Failed,
    }
}

fn task_status_to_trans_task_status(task_status: TaskStatus) -> TransTaskStatus {
    match task_status {
        TaskStatus::Running => TransTaskStatus::Running,
        TaskStatus::Stopped | TaskStatus::Paused => TransTaskStatus::Stopped,
        TaskStatus::Finished => TransTaskStatus::Finished,
        TaskStatus::Failed => TransTaskStatus::Failed,
    }
}

pub(crate) struct LocalTransService {
    download_tasks: Arc<DownloadTaskManager>,
    publish_manager: Arc<PublishManager>,
    noc: NamedObjectCacheRef,
    bdt_stack: StackGuard,

    file_recorder: FileRecorder,
    ood_resolver: OodResolver,
    chunk_manager: Arc<ChunkManager>,
    tracker: Box<dyn TrackerCache>,
    ndc: Box<dyn NamedDataCache>,
}

impl Clone for LocalTransService {
    fn clone(&self) -> Self {
        Self {
            download_tasks: self.download_tasks.clone(),
            publish_manager: self.publish_manager.clone(),
            noc: self.noc.clone(),
            bdt_stack: self.bdt_stack.clone(),
            file_recorder: self.file_recorder.clone(),
            ood_resolver: self.ood_resolver.clone(),
            chunk_manager: self.chunk_manager.clone(),
            tracker: self.tracker.clone(),
            ndc: self.ndc.clone(),
        }
    }
}

impl LocalTransService {
    pub fn new(
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        ood_resolver: OodResolver,
        chunk_manager: Arc<ChunkManager>,
        task_manager: Arc<TaskManager>,
        trans_store: Arc<TransStore>,
    ) -> Self {
        let tasks = DownloadTaskManager::new(
            bdt_stack.clone(),
            chunk_manager.clone(),
            task_manager.clone(),
            trans_store,
        );
        let publish_manager = PublishManager::new(
            task_manager.clone(),
            ndc.clone(),
            tracker.clone(),
            noc.clone(),
            bdt_stack.local_device_id().clone(),
        );

        let file_recorder = FileRecorder::new(
            ndc.clone(),
            tracker.clone(),
            noc.clone(),
            bdt_stack.local_device_id().to_owned(),
        );

        Self {
            download_tasks: Arc::new(tasks),
            publish_manager: Arc::new(publish_manager),
            noc,
            bdt_stack,
            file_recorder,
            ood_resolver,
            chunk_manager,
            tracker,
            ndc,
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        // 开启所有下载和上传任务
        // self.load_all_task().await;

        Ok(())
    }

    // file的owner可能是device或者people，如果是device，那么不一定是ood，所以需要先从owner开始查找ood
    async fn resolve_ood(&self, object_id: &ObjectId) -> BuckyResult<Vec<DeviceId>> {
        match self.ood_resolver.resolve_ood(&object_id, None).await {
            Ok(list) => {
                if list.len() > 0 {
                    info!(
                        "resole ood for file's owner success: owner={}, ood={:?}",
                        object_id, list
                    );
                    Ok(list)
                } else {
                    let msg = format!(
                        "resolve ood for file's owner but not found! owner={}",
                        object_id
                    );
                    error!("{}", msg);

                    Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                }
            }
            Err(e) => {
                let msg = format!(
                    "resolve ood for file's owner failed! owner={}, {}",
                    object_id, e
                );
                error!("{}", msg);

                Err(BuckyError::new(e, msg))
            }
        }
    }

    //添加一个文件到本地ndc+tracker缓存，并根据配置开启一个上传任务
    pub async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        if req.local_path.to_string_lossy().to_string() == "".to_string() && req.file_id.is_some() {
            let file = match self
                .noc
                .get_object(&NamedObjectCacheGetObjectRequest {
                    source: RequestSourceInfo::new_local_dec(req.common.dec_id),
                    object_id: req.file_id.unwrap(),
                    last_access_rpath: None,
                })
                .await?
            {
                Some(resp) => {
                    match File::clone_from_slice(
                        resp.object.object_raw.as_slice(),
                    ) {
                        Ok(file) => Some(file),
                        Err(_) => None,
                    }
                }
                None => None,
            };

            if file.is_some() {
                let file_recorder = FileRecorder::new(
                    self.ndc.clone(),
                    self.tracker.clone(),
                    self.noc.clone(),
                    self.bdt_stack.local_device_id().clone(),
                );
                file_recorder
                    .add_file_to_ndc(file.as_ref().unwrap(), None)
                    .await?;
            } else {
                let msg = format!(
                    "can't find file {}",
                    req.file_id.as_ref().unwrap().to_string()
                );
                log::error!("{}", msg.as_str());
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
            Ok(TransPublishFileInputResponse {
                file_id: req.file_id.clone().unwrap(),
            })
        } else if req.local_path.is_file() {
            self.add_file_impl(req).await
        } else if req.local_path.is_dir() {
            self.add_dir(req).await
        } else {
            let msg = format!(
                "trans add file but not valid file or dir: {}",
                req.local_path.display()
            );
            error!("{}", msg);

            Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
        }
    }

    async fn add_file_impl(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        info!("trans recv add file request: {:?}", req);

        let dec_id = if req.common.dec_id.is_some() {
            req.common.dec_id.as_ref().unwrap().clone()
        } else {
            let msg = format!(
                "trans add file dec id is none! file={}",
                req.local_path.to_string_lossy().to_string()
            );
            error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        };

        let file = if req.file_id.is_some() {
            match self
                .noc
                .get_object(&NamedObjectCacheGetObjectRequest {
                    source: RequestSourceInfo::new_local_dec(req.common.dec_id),
                    object_id: req.file_id.unwrap(),
                    last_access_rpath: None,
                })
                .await?
            {
                Some(resp) => {
                    match File::clone_from_slice(
                        resp.object.object_raw.as_slice(),
                    ) {
                        Ok(file) => Some(file),
                        Err(_) => None,
                    }
                }
                None => None,
            }
        } else {
            None
        };
        let file_id = self
            .publish_manager
            .publish_local_file(
                req.common.source,
                dec_id,
                req.local_path.to_string_lossy().to_string(),
                req.owner.clone(),
                file,
                req.chunk_size,
            )
            .await?;

        let resp = TransPublishFileInputResponse {
            file_id: file_id.object_id().to_owned(),
        };
        info!("trans add file success! file={}", resp.file_id);

        Ok(resp)
    }

    async fn add_dir(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        info!("trans recv add dir request: {:?}", req);

        let dec_id = if req.common.dec_id.is_some() {
            req.common.dec_id.as_ref().unwrap().clone()
        } else {
            let msg = format!(
                "trans add dir dec id is none! file={}",
                req.local_path.to_string_lossy().to_string()
            );
            error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        };

        let dir_id = self
            .publish_manager
            .publish_local_dir(
                req.common.source,
                dec_id,
                req.local_path.to_string_lossy().to_string(),
                req.owner.clone(),
                req.file_id,
                req.chunk_size,
            )
            .await?;

        let resp = TransPublishFileInputResponse { file_id: dir_id };
        info!("trans add dir success! file={}", resp.file_id);

        Ok(resp)
    }

    async fn ensure_dir(local_path: &str) -> BuckyResult<()> {
        let mut path = PathBuf::from_str(local_path).unwrap();
        if !path.exists() {
            path.pop();
            if !path.is_dir() {
                info!("will create dir: {}", path.display());
                async_std::fs::create_dir_all(&path).await.map_err(|e| {
                    let msg = format!("create dir failed! dir={}, {}", path.display(), e,);
                    error!("{}", msg);

                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
            }
        }

        Ok(())
    }

    pub async fn create_task(
        &self,
        mut req: TransCreateTaskInputRequest,
    ) -> BuckyResult<TransCreateTaskInputResponse> {
        let local_path = req.local_path.to_str().unwrap();

        // 必须至少指定一个device
        if req.device_list.is_empty() {
            info!(
                "trans task device_list is empty, now will resolve from id={}...",
                req.object_id.to_string()
            );
            let device_list = self.resolve_ood(&req.object_id).await?;
            if device_list.is_empty() {
                let msg = format!(
                    "trans task device_list is empty! file_id={}",
                    req.object_id.to_string()
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
            req.device_list = device_list;
        }
        Self::ensure_dir(local_path).await?;

        let referer = BdtDataRefererInfo {
            object_id: req.object_id,
            inner_path: None, // trans-task都是file为粒度
            dec_id: req.common.dec_id,
            req_path: req.common.req_path,
            referer_object: req.common.referer_object,
            flags: req.common.flags,
        };

        let dec_id = if req.common.dec_id.is_some() {
            req.common.dec_id.as_ref().unwrap().clone()
        } else {
            let msg = format!(
                "trans create task dec id is none! file_id={}",
                req.object_id.to_string()
            );
            error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        };
        let task_id = if req.object_id.obj_type_code() == ObjectTypeCode::File {
            let object = self.get_object_from_noc(&req.object_id).await?;
            if let AnyNamedObject::Standard(StandardObject::File(file_obj)) = object.as_ref() {
                let task_id = self
                    .download_tasks
                    .create_file_task(
                        req.common.source,
                        dec_id,
                        req.context_id,
                        file_obj.clone(),
                        Some(local_path.to_string()),
                        req.device_list,
                        referer.encode_string(),
                    )
                    .await?;
                task_id
            } else {
                let msg = format!(
                    "trans create task unknown object type! file_id={}",
                    req.object_id.to_string()
                );
                error!("{}", msg.as_str());
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        } else if req.object_id.obj_type_code() == ObjectTypeCode::Chunk {
            let task_id = self
                .download_tasks
                .create_chunk_task(
                    req.common.source,
                    dec_id,
                    req.context_id,
                    ChunkId::try_from(&req.object_id)?,
                    Some(local_path.to_string()),
                    req.device_list,
                    referer.encode_string(),
                )
                .await?;
            task_id
        } else if req.object_id.obj_type_code() == ObjectTypeCode::Dir
            || req.object_id.obj_type_code() == ObjectTypeCode::ObjectMap
        {
            let msg = format!(
                "trans create task unsupport dir! file_id={}",
                req.object_id.to_string()
            );
            error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        } else {
            let msg = format!(
                "trans create task unknown object type! file_id={}",
                req.object_id.to_string()
            );
            error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        };

        if req.auto_start {
            self.download_tasks.start_task(&task_id).await?;
        }

        Ok(TransCreateTaskInputResponse {
            task_id: task_id.to_string(),
        })
    }

    pub async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()> {
        // 使用目标对象id作为task_id
        let task_id = TaskId::from_str(req.task_id.as_str())?;

        info!(
            "will control trans task: task_id={}, req={:?}",
            task_id, req
        );

        let dec_id = if req.common.dec_id.is_some() {
            req.common.dec_id.as_ref().unwrap().clone()
        } else {
            let msg = format!(
                "trans control task dec id is none! task={}",
                req.task_id.to_string()
            );
            error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        };

        let _state = match req.action {
            TransTaskControlAction::Start => {
                self.download_tasks.start_task(&task_id).await?;
            }
            TransTaskControlAction::Stop => {
                self.download_tasks.stop_task(&task_id).await?;
            }
            TransTaskControlAction::Delete => {
                self.download_tasks
                    .remove_task(&req.common.source, &dec_id, &task_id)
                    .await?;
            }
        };

        Ok(())
    }

    pub async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransTaskState> {
        let task_id = TaskId::from_str(req.task_id.as_str())?;

        let task_state = self.download_tasks.get_task_state(&task_id).await?;

        let task_state = match task_state.task_status {
            TaskStatus::Stopped => TransTaskState::Paused,
            TaskStatus::Paused => TransTaskState::Paused,
            TaskStatus::Running => {
                if task_state.sum_size == 0 {
                    TransTaskState::Downloading(TransTaskOnAirState {
                        download_percent: 0,
                        download_speed: task_state.speed as u32,
                        upload_speed: 0,
                    })
                } else {
                    TransTaskState::Downloading(TransTaskOnAirState {
                        download_percent: task_state.downloaded_progress as u32,
                        download_speed: task_state.speed as u32,
                        upload_speed: 0,
                    })
                }
            }
            TaskStatus::Finished => TransTaskState::Finished(0),
            TaskStatus::Failed => TransTaskState::Err(task_state.err_code.unwrap()),
        };

        Ok(task_state)
    }

    pub async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        if req.common.dec_id.is_none() {
            let msg = format!("query tasks need dec_id");
            log::error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }
        let task_status = if req.task_status.is_some() {
            Some(trans_task_status_to_task_status(req.task_status.unwrap()))
        } else {
            None
        };
        let task_list = self
            .download_tasks
            .get_tasks(
                &req.common.source,
                req.common.dec_id.as_ref().unwrap(),
                &req.context_id,
                task_status,
                req.range,
            )
            .await?;
        Ok(TransQueryTasksInputResponse { task_list })
    }

    async fn get_object_from_noc(&self, object_id: &ObjectId) -> BuckyResult<Arc<AnyNamedObject>> {
        // 如果没指定flags，那么使用默认值
        // let flags = req.flags.unwrap_or(0);
        let noc_req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: object_id.to_owned(),
            last_access_rpath: None,
        };

        match self.noc.get_object(&noc_req).await {
            Ok(Some(resp)) => {
                Ok(resp.object.object.unwrap())
            }
            Ok(None) => {
                let msg = format!("noc get object but not found: {}", object_id);
                debug!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
            Err(e) => Err(e),
        }
    }
}

#[async_trait::async_trait]
impl TransInputProcessor for LocalTransService {
    async fn get_context(&self, req: TransGetContextInputRequest) -> BuckyResult<TransContext> {
        if req.common.dec_id.is_none() {
            let msg = format!("get context need dec_id");
            log::error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let noc_req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: TransContext::gen_context_id(req.common.dec_id.unwrap(), req.context_name),
            last_access_rpath: None,
        };
        match self.noc.get_object(&noc_req).await {
            Ok(Some(resp)) => {
                Ok(TransContext::clone_from_slice(
                    resp.object.object_raw.as_slice(),
                )?)
            }
            Ok(None) => {
                let msg = format!("noc get object but not found: {}", noc_req.object_id);
                debug!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
            Err(e) => Err(e),
        }
    }

    async fn put_context(&self, req: TransUpdateContextInputRequest) -> BuckyResult<()> {
        let object_raw = req.context.to_vec()?;
        let object = NONObjectInfo::new_from_object_raw(object_raw)?;

        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };

        self.noc.put_object(&req).await?;
        Ok(())
    }

    async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()> {
        Self::control_task(self, req).await
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransTaskState> {
        Self::get_task_state(self, req).await
    }

    async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        Self::publish_file(self, req).await
    }

    async fn create_task(
        &self,
        req: TransCreateTaskInputRequest,
    ) -> BuckyResult<TransCreateTaskInputResponse> {
        Self::create_task(self, req).await
    }

    async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        Self::query_tasks(self, req).await
    }
}
