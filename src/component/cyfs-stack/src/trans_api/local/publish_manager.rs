use crate::root_state_api::ObjectMapNOCCacheAdapter;
use crate::trans_api::local::FileRecorder;
use crate::util_api::{BuildDirParams, BuildDirTaskStatus, BuildFileParams, BuildFileTaskStatus};
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_task_manager::*;
use cyfs_util::cache::{NamedDataCache, TrackerCache};
use sha2::Digest;
use std::path::Path;
use std::sync::Arc;
use cyfs_debug::Mutex;

#[derive(RawEncode, RawDecode)]
pub struct PublishLocalFile {
    local_path: String,
    owner: ObjectId,
    file: File,
    chunk_size: u32,
}

#[derive(RawEncode, RawDecode)]
pub enum PublishLocalFileTaskStatus {
    Stopped,
    Running,
    Finished,
    Failed(BuckyError),
}

struct PublishLocalFileTask {
    task_store: Option<Arc<dyn TaskStore>>,
    task_id: TaskId,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    noc: NamedObjectCacheRef,
    dec_id: ObjectId,
    local_path: String,
    owner: ObjectId,
    file: File,
    chunk_size: u32,
    task_state: Mutex<PublishLocalFileTaskStatus>,
}

impl PublishLocalFileTask {
    pub fn new(
        local_path: String,
        owner: ObjectId,
        file: File,
        chunk_size: u32,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        noc: NamedObjectCacheRef,
        dec_id: ObjectId,
    ) -> Self {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(PUBLISH_TASK_CATEGORY.0.to_be_bytes());
        sha256.input(PUBLISH_LOCAL_FILE_TASK.0.to_be_bytes());
        sha256.input(local_path.as_bytes());
        let task_id = TaskId::from(sha256.result());
        Self {
            task_store: None,
            task_id,
            ndc,
            tracker,
            noc,
            dec_id,
            local_path,
            owner,
            file,
            chunk_size,
            task_state: Mutex::new(PublishLocalFileTaskStatus::Stopped),
        }
    }
}

#[async_trait::async_trait]
impl Runnable for PublishLocalFileTask {
    fn get_task_id(&self) -> TaskId {
        self.task_id.clone()
    }

    fn get_task_type(&self) -> TaskType {
        PUBLISH_LOCAL_FILE_TASK
    }

    fn get_task_category(&self) -> TaskCategory {
        PUBLISH_TASK_CATEGORY
    }

    fn need_persist(&self) -> bool {
        false
    }
    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>) {
        self.task_store = Some(task_store);
    }

    async fn run(&self) -> BuckyResult<()> {
        {
            let mut state = self.task_state.lock().unwrap();
            *state = PublishLocalFileTaskStatus::Running;
        }
        let file_recorder = FileRecorder::new(
            self.ndc.clone(),
            self.tracker.clone(),
            self.noc.clone(),
            self.dec_id.clone(),
        );

        file_recorder
            .record_file_chunk_list(Path::new(self.local_path.as_str()), &self.file)
            .await
            .map_err(|e| {
                let mut state = self.task_state.lock().unwrap();
                *state = PublishLocalFileTaskStatus::Failed(e.clone());
                e
            })?;
        file_recorder
            .add_file_to_ndc(&self.file, None)
            .await
            .map_err(|e| {
                let mut state = self.task_state.lock().unwrap();
                *state = PublishLocalFileTaskStatus::Failed(e.clone());
                e
            })?;

        let mut state = self.task_state.lock().unwrap();
        *state = PublishLocalFileTaskStatus::Finished;

        Ok(())
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        let state = self.task_state.lock().unwrap();
        Ok(state.to_vec()?)
    }
}

struct PublishLocalFileTaskFactory {
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    noc: NamedObjectCacheRef,
}

impl PublishLocalFileTaskFactory {
    pub(crate) fn new(
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        noc: NamedObjectCacheRef,
    ) -> Self {
        Self {
            ndc,
            tracker,
            noc,
        }
    }
}

#[async_trait::async_trait]
impl TaskFactory for PublishLocalFileTaskFactory {
    fn get_task_type(&self) -> TaskType {
        PUBLISH_LOCAL_FILE_TASK
    }

    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>> {
        let params = PublishLocalFile::clone_from_slice(params)?;

        let runnable = PublishLocalFileTask::new(
            params.local_path,
            params.owner,
            params.file,
            params.chunk_size,
            self.ndc.clone(),
            self.tracker.clone(),
            self.noc.clone(),
            cyfs_core::get_system_dec_app().to_owned(),
        );
        Ok(Box::new(RunnableTask::new(runnable)))
    }

    async fn restore(
        &self,
        _task_status: TaskStatus,
        params: &[u8],
        _data: &[u8],
    ) -> BuckyResult<Box<dyn Task>> {
        let params = PublishLocalFile::clone_from_slice(params)?;

        let runnable = PublishLocalFileTask::new(
            params.local_path,
            params.owner,
            params.file,
            params.chunk_size,
            self.ndc.clone(),
            self.tracker.clone(),
            self.noc.clone(),
            cyfs_core::get_system_dec_app().to_owned(),
        );
        Ok(Box::new(RunnableTask::new(runnable)))
    }
}

#[derive(RawEncode, RawDecode)]
pub struct PublishLocalDir {
    local_path: String,
    root_id: ObjectId,
}

struct PublishLocalDirTask {
    task_store: Option<Arc<dyn TaskStore>>,
    task_id: TaskId,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    noc: NamedObjectCacheRef,
    dec_id: ObjectId,
    local_path: String,
    root_id: ObjectId,
    task_state: Mutex<PublishLocalFileTaskStatus>,
}

impl PublishLocalDirTask {
    pub fn new(
        local_path: String,
        root_id: ObjectId,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        noc: NamedObjectCacheRef,
        dec_id: ObjectId,
    ) -> Self {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(PUBLISH_TASK_CATEGORY.0.to_be_bytes());
        sha256.input(PUBLISH_LOCAL_DIR_TASK.0.to_be_bytes());
        sha256.input(local_path.as_bytes());
        let task_id = TaskId::from(sha256.result());
        Self {
            task_store: None,
            task_id,
            ndc,
            tracker,
            noc,
            dec_id,
            local_path,
            root_id,
            task_state: Mutex::new(PublishLocalFileTaskStatus::Stopped),
        }
    }

    async fn publish(&self) -> BuckyResult<()> {
        let noc = ObjectMapNOCCacheAdapter::new_noc_cache(self.noc.clone());
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());
        let root = cache.get_object_map(&self.root_id).await?;
        let root_path = Path::new(self.local_path.as_str());

        if root.is_some() {
            let mut it = ObjectMapPathIterator::new(
                root.unwrap(),
                cache,
                ObjectMapPathIteratorOption::new(true, false),
            )
            .await;
            while !it.is_end() {
                let list = it.next(10).await?;
                for item in list.list.iter() {
                    if let ObjectMapContentItem::Map((file_name, object_id)) = &item.value {
                        if object_id.obj_type_code() == ObjectTypeCode::File {
                            let resp = self
                                .noc
                                .get_object(&NamedObjectCacheGetObjectRequest {
                                    source: RequestSourceInfo::new_local_dec(Some(self.dec_id.clone())),
                                    object_id: object_id.clone(),
                                    last_access_rpath: None,
                                })
                                .await?;
                            if resp.is_some() {
                                let file = File::clone_from_slice(
                                    resp.unwrap().object.object_raw.as_slice(),
                                )?;
                                let file_recorder = FileRecorder::new(
                                    self.ndc.clone(),
                                    self.tracker.clone(),
                                    self.noc.clone(),
                                    self.dec_id.clone(),
                                );

                                let sub_path = Path::new(item.path.as_str());
                                let file_path = root_path
                                    .join(sub_path.strip_prefix("/").unwrap())
                                    .join(file_name);
                                log::info!(
                                    "publish file {}",
                                    file_path.to_string_lossy().to_string()
                                );
                                file_recorder
                                    .record_file_chunk_list(file_path.as_path(), &file)
                                    .await?;
                                file_recorder.add_file_to_ndc(&file, None).await?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Runnable for PublishLocalDirTask {
    fn get_task_id(&self) -> TaskId {
        self.task_id.clone()
    }

    fn get_task_type(&self) -> TaskType {
        PUBLISH_LOCAL_DIR_TASK
    }

    fn get_task_category(&self) -> TaskCategory {
        PUBLISH_TASK_CATEGORY
    }

    fn need_persist(&self) -> bool {
        false
    }

    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>) {
        self.task_store = Some(task_store);
    }

    async fn run(&self) -> BuckyResult<()> {
        {
            let mut state = self.task_state.lock().unwrap();
            *state = PublishLocalFileTaskStatus::Running;
        }
        match self.publish().await {
            Ok(_) => {
                let mut state = self.task_state.lock().unwrap();
                *state = PublishLocalFileTaskStatus::Finished;
                Ok(())
            }
            Err(e) => {
                let mut state = self.task_state.lock().unwrap();
                *state = PublishLocalFileTaskStatus::Failed(e.clone());
                Err(e)
            }
        }
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        let state = self.task_state.lock().unwrap();
        state.to_vec()
    }
}

struct PublishLocalDirTaskFactory {
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    noc: NamedObjectCacheRef,
}

impl PublishLocalDirTaskFactory {
    pub(crate) fn new(
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        noc: NamedObjectCacheRef,
    ) -> Self {
        Self {
            ndc,
            tracker,
            noc,
        }
    }
}

#[async_trait::async_trait]
impl TaskFactory for PublishLocalDirTaskFactory {
    fn get_task_type(&self) -> TaskType {
        PUBLISH_LOCAL_DIR_TASK
    }

    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>> {
        let params = PublishLocalDir::clone_from_slice(params)?;

        let runnable = PublishLocalDirTask::new(
            params.local_path,
            params.root_id,
            self.ndc.clone(),
            self.tracker.clone(),
            self.noc.clone(),
            cyfs_core::get_system_dec_app().to_owned(),
        );
        Ok(Box::new(RunnableTask::new(runnable)))
    }

    async fn restore(
        &self,
        _task_status: TaskStatus,
        params: &[u8],
        _data: &[u8],
    ) -> BuckyResult<Box<dyn Task>> {
        let params = PublishLocalDir::clone_from_slice(params)?;

        let runnable = PublishLocalDirTask::new(
            params.local_path,
            params.root_id,
            self.ndc.clone(),
            self.tracker.clone(),
            self.noc.clone(),
            cyfs_core::get_system_dec_app().to_owned(),
        );
        Ok(Box::new(RunnableTask::new(runnable)))
    }
}

pub struct PublishManager {
    task_manager: Arc<TaskManager>,
    device_id: DeviceId,
}

impl PublishManager {
    pub fn new(
        task_manager: Arc<TaskManager>,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        noc: NamedObjectCacheRef,
        device_id: DeviceId,
    ) -> Self {
        task_manager
            .register_task_factory(PublishLocalDirTaskFactory::new(
                ndc.clone(),
                tracker.clone(),
                noc.clone(),
            ))
            .unwrap();
        task_manager
            .register_task_factory(PublishLocalFileTaskFactory::new(
                ndc,
                tracker,
                noc,
            ))
            .unwrap();

        let tmp_task_manager = task_manager.clone();
        async_std::task::spawn(async move {
            if let Err(e) = Self::clear_finished_task(tmp_task_manager).await {
                log::error!("clear finished publish task failed.{}", e);
            }
        });

        Self {
            task_manager,
            device_id,
        }
    }

    pub async fn clear_finished_task(task_manager: Arc<TaskManager>) -> BuckyResult<()> {
        let list = task_manager
            .get_tasks_by_category(PUBLISH_TASK_CATEGORY)
            .await?;
        for (task_id, _, task_status, _, _) in list.iter() {
            if *task_status == TaskStatus::Finished {
                task_manager.remove_task_by_task_id(task_id).await?;
            }
        }
        Ok(())
    }

    pub async fn publish_local_file(
        &self,
        source: DeviceId,
        dec_id: ObjectId,
        local_path: String,
        owner: ObjectId,
        file: Option<File>,
        chunk_size: u32,
    ) -> BuckyResult<FileId> {
        let file = if file.is_none() {
            let params = BuildFileParams {
                local_path: local_path.clone(),
                owner,
                chunk_size,
            };
            let task_id = self
                .task_manager
                .create_task(dec_id.clone(), source.clone(), BUILD_FILE_TASK, params)
                .await?;
            self.task_manager.start_task(&task_id).await?;
            self.task_manager.check_and_waiting_stop(&task_id).await;
            let detail_status = BuildFileTaskStatus::clone_from_slice(
                self.task_manager
                    .get_task_detail_status(&task_id)
                    .await?
                    .as_slice(),
            )?;
            self.task_manager
                .remove_task(&dec_id, &source, &task_id)
                .await?;
            if let BuildFileTaskStatus::Finished(file) = detail_status {
                file
            } else {
                return Err(BuckyError::new(
                    BuckyErrorCode::Failed,
                    format!("publish local file {} failed", local_path),
                ));
            }
        } else {
            file.unwrap()
        };

        let file_id = file.desc().file_id();
        let params = PublishLocalFile {
            local_path: local_path.clone(),
            owner,
            file,
            chunk_size,
        };

        let task_id = self
            .task_manager
            .create_task(
                dec_id.clone(),
                source.clone(),
                PUBLISH_LOCAL_FILE_TASK,
                params,
            )
            .await?;
        self.task_manager.start_task(&task_id).await?;
        self.task_manager.check_and_waiting_stop(&task_id).await;
        let detail_status = self.task_manager.get_task_detail_status(&task_id).await?;
        self.task_manager
            .remove_task(&dec_id, &source, &task_id)
            .await?;
        let state = PublishLocalFileTaskStatus::clone_from_slice(detail_status.as_slice())?;
        if let PublishLocalFileTaskStatus::Finished = state {
            Ok(file_id)
        } else {
            Err(BuckyError::new(
                BuckyErrorCode::Failed,
                format!("publish local file {} failed", local_path),
            ))
        }
    }

    pub async fn publish_local_dir(
        &self,
        source: DeviceId,
        dec_id: ObjectId,
        local_path: String,
        owner: ObjectId,
        dir: Option<ObjectId>,
        chunk_size: u32,
    ) -> BuckyResult<ObjectId> {
        let root_id = if dir.is_none() {
            let params = BuildDirParams {
                local_path: local_path.clone(),
                owner,
                chunk_size,
                device_id: self.device_id.clone(),
            };

            let task_id = self
                .task_manager
                .create_task(dec_id.clone(), source.clone(), BUILD_DIR_TASK, params)
                .await?;
            self.task_manager.start_task(&task_id).await?;
            self.task_manager.check_and_waiting_stop(&task_id).await;
            let detail_status = BuildDirTaskStatus::clone_from_slice(
                self.task_manager
                    .get_task_detail_status(&task_id)
                    .await?
                    .as_slice(),
            )?;
            self.task_manager
                .remove_task(&dec_id, &source, &task_id)
                .await?;
            if let BuildDirTaskStatus::Finished(object_id) = detail_status {
                object_id
            } else {
                return Err(BuckyError::new(
                    BuckyErrorCode::Failed,
                    format!("publish local dir {} failed", local_path),
                ));
            }
        } else {
            dir.unwrap()
        };

        let params = PublishLocalDir {
            local_path: local_path.clone(),
            root_id: root_id.clone(),
        };

        let task_id = self
            .task_manager
            .create_task(
                dec_id.clone(),
                source.clone(),
                PUBLISH_LOCAL_DIR_TASK,
                params,
            )
            .await?;
        self.task_manager.start_task(&task_id).await?;
        self.task_manager.check_and_waiting_stop(&task_id).await;
        let detail_status = self.task_manager.get_task_detail_status(&task_id).await?;
        self.task_manager
            .remove_task(&dec_id, &source, &task_id)
            .await?;
        let state = PublishLocalFileTaskStatus::clone_from_slice(detail_status.as_slice())?;
        if let PublishLocalFileTaskStatus::Finished = state {
            Ok(root_id)
        } else {
            Err(BuckyError::new(
                BuckyErrorCode::Failed,
                format!("publish local dir {} failed", local_path),
            ))
        }
    }
}
