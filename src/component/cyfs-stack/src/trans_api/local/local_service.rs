use crate::ndn::TaskGroupHelper;
use crate::resolver::OodResolver;
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::{StackGuard, self};
use cyfs_lib::*;

use crate::trans::{TransInputProcessor, TransInputProcessorRef};
use crate::trans_api::local::FileRecorder;
use crate::trans_api::{DownloadTaskManager, PublishManager, TransStore};
use cyfs_base::File;
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

    ood_resolver: OodResolver,
    named_data_components: NamedDataComponents,
}

impl Clone for LocalTransService {
    fn clone(&self) -> Self {
        Self {
            download_tasks: self.download_tasks.clone(),
            publish_manager: self.publish_manager.clone(),
            noc: self.noc.clone(),
            bdt_stack: self.bdt_stack.clone(),
            ood_resolver: self.ood_resolver.clone(),
            named_data_components: self.named_data_components.clone(),
        }
    }
}

impl LocalTransService {
    pub fn new(
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
        named_data_components: &NamedDataComponents,
        ood_resolver: OodResolver,
        task_manager: Arc<TaskManager>,
        trans_store: Arc<TransStore>,
    ) -> Self {
        let tasks = DownloadTaskManager::new(
            bdt_stack.clone(),
            named_data_components,
            task_manager.clone(),
            trans_store,
        );
        let publish_manager = PublishManager::new(
            task_manager.clone(),
            named_data_components.ndc.clone(),
            named_data_components.tracker.clone(),
            noc.clone(),
            bdt_stack.local_device_id().clone(),
        );

        Self {
            download_tasks: Arc::new(tasks),
            publish_manager: Arc::new(publish_manager),
            noc,
            bdt_stack,
            ood_resolver,
            named_data_components: named_data_components.to_owned(),
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
                    source: req.common.source.clone(),
                    object_id: req.file_id.unwrap(),
                    last_access_rpath: None,
                })
                .await?
            {
                Some(resp) => match File::clone_from_slice(resp.object.object_raw.as_slice()) {
                    Ok(file) => Some(file),
                    Err(_) => None,
                },
                None => None,
            };

            if file.is_some() {
                let file_recorder = FileRecorder::new(
                    self.named_data_components.ndc.clone(),
                    self.named_data_components.tracker.clone(),
                    self.noc.clone(),
                    req.common.source.dec.clone(),
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

        let file = if req.file_id.is_some() {
            match self
                .noc
                .get_object(&NamedObjectCacheGetObjectRequest {
                    source: req.common.source.clone(),
                    object_id: req.file_id.unwrap(),
                    last_access_rpath: None,
                })
                .await?
            {
                Some(resp) => match File::clone_from_slice(resp.object.object_raw.as_slice()) {
                    Ok(file) => Some(file),
                    Err(_) => None,
                },
                None => None,
            }
        } else {
            None
        };
        let file_id = self
            .publish_manager
            .publish_local_file(
                req.common.source.zone.device.unwrap(),
                req.common.source.dec,
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

        let dir_id = self
            .publish_manager
            .publish_local_dir(
                req.common.source.zone.device.unwrap(),
                req.common.source.dec,
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
            // FIXME: set target field from o link
            target: None,
            object_id: req.object_id,
            inner_path: None, // trans-task都是file为粒度
            dec_id: Some(req.common.source.dec.clone()),
            req_path: req.common.req_path,
            referer_object: req.common.referer_object,
            flags: req.common.flags,
        };

        let task_id = if req.object_id.obj_type_code() == ObjectTypeCode::File {
            let object = self
                .get_object_from_noc(&req.common.source, &req.object_id)
                .await?;
            if let AnyNamedObject::Standard(StandardObject::File(file_obj)) = object.as_ref() {
                let task_id = self
                    .download_tasks
                    .create_file_task(
                        req.common.source.zone.device.unwrap(),
                        req.common.source.dec,
                        req.group,
                        req.context,
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
                    req.common.source.zone.device.unwrap(),
                    req.common.source.dec,
                    req.group,
                    req.context,
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

        let _state = match req.action {
            TransTaskControlAction::Start => {
                self.download_tasks.start_task(&task_id).await?;
            }
            TransTaskControlAction::Stop => {
                self.download_tasks.stop_task(&task_id).await?;
            }
            TransTaskControlAction::Delete => {
                self.download_tasks
                    .remove_task(
                        req.common.source.zone.device.as_ref().unwrap(),
                        &req.common.source.dec,
                        &task_id,
                    )
                    .await?;
            }
        };

        Ok(())
    }

    pub async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransGetTaskStateInputResponse> {
        let task_id = TaskId::from_str(req.task_id.as_str())?;

        let task_state = self.download_tasks.get_task_state(&task_id).await?;

        let state = match task_state.task_status {
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

        let resp = TransGetTaskStateInputResponse {
            state,
            group: task_state.group,
        };

        Ok(resp)
    }

    pub async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        let task_status = if req.task_status.is_some() {
            Some(trans_task_status_to_task_status(req.task_status.unwrap()))
        } else {
            None
        };
        let task_list = self
            .download_tasks
            .get_tasks(
                req.common.source.zone.device.as_ref().unwrap(),
                &req.common.source.dec,
                task_status,
                req.range,
            )
            .await?;
        Ok(TransQueryTasksInputResponse { task_list })
    }

    async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateInputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateInputResponse> {
        let group = TaskGroupHelper::check_and_fix(&req.common.source.dec, req.group);

        use cyfs_bdt::{DownloadTask, UploadTask, NdnTask};
        let task = match req.group_type {
            TransTaskGroupType::Download => self.bdt_stack.ndn().root_task().download().sub_task(&group).map(|task| task.clone_as_task()), 
            TransTaskGroupType::Upload => self.bdt_stack.ndn().root_task().upload().sub_task(&group).map(|task| task.clone_as_task()), 
        }.ok_or_else(|| {
            let msg = format!("get task group but ot found! group={}", group);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        let mut resp = TransGetTaskGroupStateInputResponse {
            state: task.state(),
            control_state: task.control_state(),
            speed: None,
            cur_speed: task.cur_speed(),
            history_speed: task.history_speed(),
        };

        if let Some(tm) = req.speed_when {
            resp.speed = Some(task.cur_speed());
        }

        Ok(resp)
    }

    async fn control_task_group(
        &self,
        req: TransControlTaskGroupInputRequest,
    ) -> BuckyResult<TransControlTaskGroupInputResponse> {
        let group = TaskGroupHelper::check_and_fix(&req.common.source.dec, req.group);

        use cyfs_bdt::{DownloadTask, UploadTask, NdnTask};
        let task: Box<dyn NdnTask> = match req.group_type {
            TransTaskGroupType::Download => self.bdt_stack.ndn().root_task().download().sub_task(&group).map(|task| task.clone_as_task()),
            TransTaskGroupType::Upload => self.bdt_stack.ndn().root_task().upload().sub_task(&group).map(|task| task.clone_as_task()), 
        }.ok_or_else(|| {
            let msg = format!("get task group but ot found! group={}", group);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        let control_state = match req.action {
            TransTaskGroupControlAction::Pause => task.pause(),
            TransTaskGroupControlAction::Resume => task.resume(),
            TransTaskGroupControlAction::Cancel => task.cancel(),
            TransTaskGroupControlAction::Close => task.close().map(|_| task.control_state())
        }?;

        let resp = TransControlTaskGroupInputResponse { control_state };

        Ok(resp)
    }

    async fn get_object_from_noc(
        &self,
        source: &RequestSourceInfo,
        object_id: &ObjectId,
    ) -> BuckyResult<Arc<AnyNamedObject>> {
        // 如果没指定flags，那么使用默认值
        // let flags = req.flags.unwrap_or(0);
        let noc_req = NamedObjectCacheGetObjectRequest {
            source: source.clone(),
            object_id: object_id.to_owned(),
            last_access_rpath: None,
        };

        match self.noc.get_object(&noc_req).await {
            Ok(Some(resp)) => Ok(resp.object.object.unwrap()),
            Ok(None) => {
                let msg = format!("noc get object but not found: {}", object_id);
                debug!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
            Err(e) => Err(e),
        }
    }

    pub fn clone_processor(&self) -> TransInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }
}

#[async_trait::async_trait]
impl TransInputProcessor for LocalTransService {
    async fn get_context(
        &self,
        req: TransGetContextInputRequest,
    ) -> BuckyResult<TransGetContextInputResponse> {
        let ret = if let Some(id) = &req.context_id {
            self.named_data_components
                .context_manager
                .get_context(id)
                .await
        } else if let Some(context_path) = &req.context_path {
            if context_path.starts_with('$') {
                self.named_data_components
                    .context_manager
                    .get_context_by_path(None, context_path.as_str())
                    .await
            } else {
                self.named_data_components
                    .context_manager
                    .get_context_by_path(Some(req.common.source.dec.clone()), context_path.as_str())
                    .await
            }
        } else {
            let msg = format!(
                "context_id and context_path must specify one of them for get_context request!"
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        };

        if ret.is_none() {
            let msg = format!(
                "get context but not found: id={:?}, path={:?}",
                req.context_id, req.context_path
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        Ok(TransGetContextInputResponse {
            context: ret.unwrap().object.clone(),
        })
    }

    async fn put_context(&self, req: TransUpdateContextInputRequest) -> BuckyResult<()> {
        self.named_data_components
            .context_manager
            .put_context(req.common.source, req.context, req.access)
            .await
    }

    async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()> {
        Self::control_task(self, req).await
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransGetTaskStateInputResponse> {
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

    async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateInputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateInputResponse> {
        Self::get_task_group_state(self, req).await
    }

    async fn control_task_group(
        &self,
        req: TransControlTaskGroupInputRequest,
    ) -> BuckyResult<TransControlTaskGroupInputResponse> {
        Self::control_task_group(self, req).await
    }
}
