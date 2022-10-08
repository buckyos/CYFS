use cyfs_base::*;
use cyfs_lib::*;
use cyfs_task_manager::*;
use cyfs_util::*;
use sha2::Digest;
use std::sync::Arc;

#[derive(RawEncode, RawDecode)]
pub struct BuildFileParams {
    pub local_path: String,
    pub owner: ObjectId,
    pub chunk_size: u32,
}

pub struct BuildFileTaskFactory {
    noc: NamedObjectCacheRef,
    ndc: Box<dyn NamedDataCache>,
}

impl BuildFileTaskFactory {
    pub fn new(noc: NamedObjectCacheRef, ndc: Box<dyn NamedDataCache>) -> Self {
        Self { noc, ndc }
    }
}

#[async_trait::async_trait]
impl TaskFactory for BuildFileTaskFactory {
    fn get_task_type(&self) -> TaskType {
        BUILD_FILE_TASK
    }

    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>> {
        let params = BuildFileParams::clone_from_slice(params)?;
        Ok(Box::new(RunnableTask::new(BuildFileTask::new(
            params.local_path,
            params.owner,
            params.chunk_size,
            self.noc.clone(),
            self.ndc.clone(),
        ))))
    }

    async fn restore(
        &self,
        _task_status: TaskStatus,
        params: &[u8],
        data: &[u8],
    ) -> BuckyResult<Box<dyn Task>> {
        let params = BuildFileParams::clone_from_slice(params)?;
        let task_state = if data.len() > 0 {
            FileTaskState::clone_from_slice(data)?
        } else {
            FileTaskState::new()
        };

        Ok(Box::new(RunnableTask::new(BuildFileTask::restore(
            params.local_path,
            params.owner,
            params.chunk_size,
            task_state,
            self.noc.clone(),
            self.ndc.clone(),
        ))))
    }
}

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) struct FileTaskState {
    pub hash_pos: u64,
    pub hash_state: Vec<u8>,
    pub pos: u64,
    pub chunk_list: Vec<ChunkId>,
}

#[derive(Clone)]
struct TaskState {
    pub state: FileTaskState,
    pub status: BuildFileTaskStatus,
    pub task_id: TaskId,
    pub task_store: Option<Arc<dyn TaskStore>>,
}

impl TaskState {
    pub fn new(state: FileTaskState, task_id: TaskId) -> Self {
        Self {
            state,
            status: BuildFileTaskStatus::Stopped,
            task_id,
            task_store: None,
        }
    }

    pub fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>) {
        self.task_store = Some(task_store);
    }
}

#[async_trait::async_trait]
impl FileObjectBuilderState for TaskState {
    async fn get_cur_state(&self) -> BuckyResult<(u64, (u64, [u32; 8]), Vec<ChunkId>)> {
        Ok((
            self.state.pos,
            self.state.hash_state(),
            self.state.chunk_list.clone(),
        ))
    }

    async fn update(
        &mut self,
        pos: u64,
        hash_state: (u64, &[u32; 8]),
        chunk_id: ChunkId,
    ) -> BuckyResult<()> {
        self.state.pos = pos;
        self.state.set_hash_state(hash_state);
        self.state.chunk_list.push(chunk_id);
        if self.task_store.is_some() {
            self.task_store
                .as_ref()
                .unwrap()
                .save_task_data(&self.task_id, self.state.to_vec()?)
                .await?;
        }
        Ok(())
    }
}

impl FileTaskState {
    pub fn new() -> Self {
        let mut hash_state = Vec::new();
        hash_state.resize(32, 0);
        Self {
            hash_pos: 0,
            hash_state,
            pos: 0,
            chunk_list: vec![],
        }
    }

    pub fn set_hash_state(&mut self, state: (u64, &[u32; 8])) {
        unsafe {
            std::ptr::copy(
                state.1.as_ptr() as *const u8,
                self.hash_state.as_mut_ptr(),
                32,
            );
        }
        self.hash_pos = state.0;
    }

    pub fn set_pos(&mut self, pos: u64) {
        self.pos = pos;
    }

    pub fn hash_state(&self) -> (u64, [u32; 8]) {
        let mut state = [0u32; 8];
        unsafe {
            std::ptr::copy(self.hash_state.as_ptr(), state.as_mut_ptr() as *mut u8, 32);
        }
        (self.hash_pos, state)
    }

    pub fn pos(&self) -> u64 {
        self.pos
    }

    pub fn add_chunk(&mut self, chunk_id: ChunkId) {
        self.chunk_list.push(chunk_id)
    }
}

#[derive(RawEncode, RawDecode, Clone)]
pub enum BuildFileTaskStatus {
    Stopped,
    Running,
    Finished(File),
    Failed(BuckyError),
}

pub struct BuildFileTask {
    task_id: TaskId,
    task_store: Option<Arc<dyn TaskStore>>,
    task_state: FileObjectBuilderStateWrapper<TaskState>,
    local_path: String,
    owner: ObjectId,
    chunk_size: u32,
    noc: NamedObjectCacheRef,
    ndc: Box<dyn NamedDataCache>,
}

impl BuildFileTask {
    pub(crate) fn new(
        local_path: String,
        owner: ObjectId,
        chunk_size: u32,
        noc: NamedObjectCacheRef,
        ndc: Box<dyn NamedDataCache>,
    ) -> Self {
        let mut sha2 = sha2::Sha256::new();
        sha2.input(local_path.as_bytes());
        sha2.input(owner.to_string().as_bytes());
        sha2.input(chunk_size.to_be_bytes());
        sha2.input(BUILD_FILE_TASK.into().to_be_bytes());
        let task_id: TaskId = sha2.result().into();
        Self {
            task_id: task_id.clone(),
            task_store: None,
            task_state: FileObjectBuilderStateWrapper::new(TaskState::new(
                FileTaskState::new(),
                task_id,
            )),
            local_path,
            owner,
            chunk_size,
            noc,
            ndc,
        }
    }

    pub(crate) fn restore(
        local_path: String,
        owner: ObjectId,
        chunk_size: u32,
        task_state: FileTaskState,
        noc: NamedObjectCacheRef,
        ndc: Box<dyn NamedDataCache>,
    ) -> Self {
        let mut sha2 = sha2::Sha256::new();
        sha2.input(local_path.as_bytes());
        sha2.input(owner.to_string().as_bytes());
        sha2.input(chunk_size.to_be_bytes());
        sha2.input(BUILD_FILE_TASK.into().to_be_bytes());
        let task_id: TaskId = sha2.result().into();
        Self {
            task_id: task_id.clone(),
            task_store: None,
            task_state: FileObjectBuilderStateWrapper::new(TaskState::new(task_state, task_id)),
            local_path,
            owner,
            chunk_size,
            noc,
            ndc,
        }
    }
}

#[async_trait::async_trait]
impl Runnable for BuildFileTask {
    fn get_task_id(&self) -> TaskId {
        self.task_id.clone()
    }

    fn get_task_type(&self) -> TaskType {
        BUILD_FILE_TASK
    }

    fn get_task_category(&self) -> TaskCategory {
        BUILD_FILE_TASK_CATEGORY
    }

    fn need_persist(&self) -> bool {
        false
    }

    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>) {
        self.task_store = Some(task_store.clone());
        self.task_state
            .get_state_mut()
            .await
            .set_task_store(task_store);
    }

    async fn run(&self) -> BuckyResult<()> {
        self.task_state.get_state_mut().await.status = BuildFileTaskStatus::Running;
        let builder = FileObjectBuilder::<TaskState>::new(
            self.local_path.clone(),
            self.owner.clone(),
            self.chunk_size,
            None,
        );
        let file = match builder.build().await {
            Ok(file) => file,
            Err(e) => {
                self.task_state.get_state_mut().await.state = FileTaskState::new();
                if self.task_store.is_some() {
                    self.task_store
                        .as_ref()
                        .unwrap()
                        .save_task_data(&self.task_id, Vec::new())
                        .await?;
                }
                self.task_state.get_state_mut().await.status =
                    BuildFileTaskStatus::Failed(e.clone());
                return Err(e);
            }
        };
        let query_ret = self
            .ndc
            .get_file_by_hash(&GetFileByHashRequest {
                hash: file.desc().content().hash().to_string(),
                flags: 0,
            })
            .await?;
        if query_ret.is_some() {
            let file = self
                .noc
                .get_object(&NamedObjectCacheGetObjectRequest {
                    last_access_rpath: None,
                    source: RequestSourceInfo::new_local_system(),
                    object_id: query_ret.unwrap().file_id.object_id().clone(),
                })
                .await?;
            if let Some(file) = file {
                if let Ok(file) = File::clone_from_slice(file.object.object_raw.as_slice()) {
                    self.task_state.get_state_mut().await.state = FileTaskState::new();
                    if self.task_store.is_some() {
                        self.task_store
                            .as_ref()
                            .unwrap()
                            .save_task_data(&self.task_id, Vec::new())
                            .await?;
                    }
                    self.task_state.get_state_mut().await.status =
                        BuildFileTaskStatus::Finished(file);
                    return Ok(());
                }
            }
        }

        let object_raw = file.to_vec()?;
        let object = NONObjectInfo::new_from_object_raw(object_raw)?;

        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };

        match self.noc.put_object(&req).await {
            Ok(_) => {
                self.task_state.get_state_mut().await.state = FileTaskState::new();
                if self.task_store.is_some() {
                    self.task_store
                        .as_ref()
                        .unwrap()
                        .save_task_data(&self.task_id, Vec::new())
                        .await?;
                }
                self.task_state.get_state_mut().await.status = BuildFileTaskStatus::Finished(file);
                Ok(())
            }
            Err(e) => {
                self.task_state.get_state_mut().await.state = FileTaskState::new();
                if self.task_store.is_some() {
                    self.task_store
                        .as_ref()
                        .unwrap()
                        .save_task_data(&self.task_id, Vec::new())
                        .await?;
                }
                self.task_state.get_state_mut().await.status =
                    BuildFileTaskStatus::Failed(e.clone());
                Err(e)
            }
        }
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        self.task_state.get_state_mut().await.status.to_vec()
    }
}

#[cfg(test)]
mod build_file_task_test {
    use crate::util_api::local::{BuildFileParams, BuildFileTaskFactory, BuildFileTaskStatus};
    use cyfs_base::{
        BuckyResult, ChunkState, DeviceId, NamedObject, ObjectDesc, ObjectId, RawFrom,
    };
    use cyfs_lib::*;
    use cyfs_task_manager::test_task_manager::create_test_task_manager;
    use cyfs_task_manager::BUILD_FILE_TASK;
    use futures::AsyncWriteExt;
    use std::path::Path;
    use std::time::Duration;
    use std::sync::Arc;

    struct MemoryNoc {}

    #[async_trait::async_trait]
    impl NamedObjectCache for MemoryNoc {
        async fn put_object(
            &self,
            req: &NamedObjectCachePutObjectRequest,
        ) -> BuckyResult<NamedObjectCachePutObjectResponse> {
            Ok(NamedObjectCachePutObjectResponse {
                result: NamedObjectCachePutObjectResult::Accept,
                update_time: None,
                expires_time: None,
            })
        }

        async fn get_object_raw(
            &self,
            req: &NamedObjectCacheGetObjectRequest,
        ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
            todo!();
        }

        async fn delete_object(
            &self,
            req: &NamedObjectCacheDeleteObjectRequest,
        ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse> {
            todo!();
        }
    
        async fn exists_object(
            &self,
            req: &NamedObjectCacheExistsObjectRequest,
        ) -> BuckyResult<NamedObjectCacheExistsObjectResponse> {
            todo!();
        }
    
        async fn update_object_meta(
            &self,
            req: &NamedObjectCacheUpdateObjectMetaRequest,
        ) -> BuckyResult<()> {
            todo!();
        }
    
        async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
            todo!();
        }
    }

    struct MemoryNDC {}

    #[async_trait::async_trait]
    impl NamedDataCache for MemoryNDC {
        fn clone(&self) -> Box<dyn NamedDataCache> {
            todo!()
        }

        async fn insert_file(&self, req: &InsertFileRequest) -> BuckyResult<()> {
            todo!()
        }

        async fn remove_file(&self, req: &RemoveFileRequest) -> BuckyResult<usize> {
            todo!()
        }

        async fn file_update_quick_hash(
            &self,
            req: &FileUpdateQuickhashRequest,
        ) -> BuckyResult<()> {
            todo!()
        }

        async fn get_file_by_hash(
            &self,
            req: &GetFileByHashRequest,
        ) -> BuckyResult<Option<FileCacheData>> {
            Ok(None)
        }

        async fn get_file_by_file_id(
            &self,
            req: &GetFileByFileIdRequest,
        ) -> BuckyResult<Option<FileCacheData>> {
            todo!()
        }

        async fn get_files_by_quick_hash(
            &self,
            req: &GetFileByQuickHashRequest,
        ) -> BuckyResult<Vec<FileCacheData>> {
            todo!()
        }

        async fn get_files_by_chunk(
            &self,
            req: &GetFileByChunkRequest,
        ) -> BuckyResult<Vec<FileCacheData>> {
            todo!()
        }

        async fn get_dirs_by_file(
            &self,
            req: &GetDirByFileRequest,
        ) -> BuckyResult<Vec<FileDirRef>> {
            todo!()
        }

        async fn insert_chunk(&self, req: &InsertChunkRequest) -> BuckyResult<()> {
            todo!()
        }

        async fn remove_chunk(&self, req: &RemoveChunkRequest) -> BuckyResult<usize> {
            todo!()
        }

        async fn update_chunk_state(
            &self,
            req: &UpdateChunkStateRequest,
        ) -> BuckyResult<ChunkState> {
            todo!()
        }

        async fn update_chunk_ref_objects(&self, req: &UpdateChunkRefsRequest) -> BuckyResult<()> {
            todo!()
        }

        async fn get_chunk(&self, req: &GetChunkRequest) -> BuckyResult<Option<ChunkCacheData>> {
            todo!()
        }

        async fn get_chunks(
            &self,
            req: &Vec<GetChunkRequest>,
        ) -> BuckyResult<Vec<Option<ChunkCacheData>>> {
            todo!()
        }

        async fn get_chunk_ref_objects(
            &self,
            req: &GetChunkRefObjectsRequest,
        ) -> BuckyResult<Vec<ChunkObjectRef>> {
            todo!()
        }

        async fn exists_chunks(&self, req: &ExistsChunkRequest) -> BuckyResult<Vec<bool>> {
            todo!();
        }
    }
    async fn gen_random_file(local_path: &Path) {
        if local_path.exists() {
            assert!(local_path.is_file());
            std::fs::remove_file(&local_path).unwrap();
        }

        let mut opt = async_std::fs::OpenOptions::new();
        opt.write(true).create(true).truncate(true);

        let mut f = opt.open(&local_path).await.unwrap();
        let mut buf: Vec<u8> = Vec::with_capacity(1024 * 1024);
        for _ in 0..1024 {
            let buf_k: Vec<u8> = (0..1024).map(|_| rand::random::<u8>()).collect();
            buf.extend_from_slice(&buf_k);
        }

        for _i in 0..20 {
            f.write_all(&buf).await.unwrap();
        }
        f.flush().await.unwrap();
    }

    #[test]
    fn test() {
        async_std::task::block_on(async move {
            let noc = Arc::new(Box::new(MemoryNoc {}) as Box<dyn NamedObjectCache>);
            let ndc = Box::new(MemoryNDC {});
            let task_manager = create_test_task_manager().await.unwrap();
            task_manager
                .register_task_factory(BuildFileTaskFactory::new(noc, ndc))
                .unwrap();

            let tmp_path = std::env::temp_dir().join("test_build_file");
            gen_random_file(tmp_path.as_path()).await;
            let local_path = tmp_path.to_string_lossy().to_string();
            let params = BuildFileParams {
                local_path: local_path.clone(),
                owner: Default::default(),
                chunk_size: 4 * 1024 * 1024,
            };
            let task = task_manager
                .create_task(
                    ObjectId::default(),
                    DeviceId::default(),
                    BUILD_FILE_TASK,
                    params,
                )
                .await
                .unwrap();
            task_manager.start_task(&task).await.unwrap();
            async_std::task::sleep(Duration::from_secs(1)).await;
            task_manager.stop_task(&task).await.unwrap();
            task_manager.start_task(&task).await.unwrap();
            task_manager.check_and_waiting_stop(&task).await;
            let resp = BuildFileTaskStatus::clone_from_slice(
                task_manager
                    .get_task_detail_status(&task)
                    .await
                    .unwrap()
                    .as_slice(),
            )
            .unwrap();
            task_manager
                .remove_task(&ObjectId::default(), &DeviceId::default(), &task)
                .await
                .unwrap();
            let file = if let BuildFileTaskStatus::Finished(file) = resp {
                file
            } else {
                assert!(false);
                return;
            };

            let params = BuildFileParams {
                local_path,
                owner: Default::default(),
                chunk_size: 4 * 1024 * 1024,
            };
            let task = task_manager
                .create_task(
                    ObjectId::default(),
                    DeviceId::default(),
                    BUILD_FILE_TASK,
                    params,
                )
                .await
                .unwrap();
            task_manager.start_task(&task).await.unwrap();
            task_manager.check_and_waiting_stop(&task).await;
            let resp1 = BuildFileTaskStatus::clone_from_slice(
                task_manager
                    .get_task_detail_status(&task)
                    .await
                    .unwrap()
                    .as_slice(),
            )
            .unwrap();
            task_manager
                .remove_task(&ObjectId::default(), &DeviceId::default(), &task)
                .await
                .unwrap();
            let file1 = if let BuildFileTaskStatus::Finished(file) = resp1 {
                file
            } else {
                assert!(false);
                return;
            };
            assert_eq!(file.desc().calculate_id(), file1.desc().calculate_id());
        });
    }
}
