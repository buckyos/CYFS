use crate::util_api::{BuildFileParams, BuildFileTaskStatus};
use async_std::task::JoinHandle;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;
use cyfs_task_manager::*;
use cyfs_util::*;
use futures::future::AbortHandle;
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(super::util_proto::BuildDirParams)]
pub struct BuildDirParams {
    pub local_path: String,
    pub owner: ObjectId,
    pub dec_id: ObjectId,
    pub chunk_size: u32,
    pub device_id: ObjectId,
    pub access: Option<u32>,
}

pub struct BuildDirTaskFactory {
    task_manager: Weak<TaskManager>,
    noc: NamedObjectCacheRef,
}

impl BuildDirTaskFactory {
    pub fn new(task_manager: Weak<TaskManager>, noc: NamedObjectCacheRef) -> Self {
        Self { task_manager, noc }
    }
}

#[async_trait::async_trait]
impl TaskFactory for BuildDirTaskFactory {
    fn get_task_type(&self) -> TaskType {
        BUILD_DIR_TASK
    }

    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>> {
        let params = BuildDirParams::clone_from_slice(params)?;
        let task = BuildDirTask::new(
            params.local_path,
            params.owner,
            params.dec_id,
            params.chunk_size,
            params.access,
            self.task_manager.clone(),
            DeviceId::try_from(params.device_id)?,
            self.noc.clone(),
        );
        Ok(Box::new(task))
    }

    async fn restore(
        &self,
        _task_status: TaskStatus,
        params: &[u8],
        data: &[u8],
    ) -> BuckyResult<Box<dyn Task>> {
        let params = BuildDirParams::clone_from_slice(params)?;
        let task_state = DirTaskState::clone_from_slice(data)?;

        let task = BuildDirTask::restore(
            params.local_path,
            params.owner,
            params.dec_id,
            params.chunk_size,
            params.access,
            self.task_manager.clone(),
            DeviceId::try_from(params.device_id)?,
            self.noc.clone(),
            task_state,
        );
        Ok(Box::new(task))
    }
}

#[derive(Clone)]
struct PathPostorderIteratorPathState {
    dir_list: Vec<PathBuf>,
    iter_dir_pos: usize,
    file_list: Vec<PathBuf>,
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct PathPostorderIteratorState {
    root_path: String,
    cur_path: Option<String>,
}

pub struct PathPostorderIterator {
    root_path: PathBuf,
    cur_path: Option<PathBuf>,
    path_states: HashMap<PathBuf, PathPostorderIteratorPathState>,
}

impl PathPostorderIterator {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            cur_path: None,
            path_states: HashMap::new(),
        }
    }

    pub fn from_state(state: PathPostorderIteratorState) -> Self {
        let mut path_states = HashMap::new();
        let root_path = PathBuf::from(state.root_path);
        let cur_path = if state.cur_path.is_some() {
            let cur_path = PathBuf::from(state.cur_path.unwrap());
            if cur_path.starts_with(root_path.as_path()) {
                Some(cur_path)
            } else {
                None
            }
        } else {
            None
        };
        if cur_path.is_some() {
            let mut cur_path = cur_path.as_ref().unwrap().to_path_buf();
            while cur_path != root_path {
                let file_name = cur_path.file_name().unwrap().to_string_lossy().to_string();
                cur_path = cur_path.parent().unwrap().to_path_buf();
                if let Ok((dir_list, file_list)) = Self::list_dir(cur_path.as_path()) {
                    let mut state_index = 0;
                    for (index, sub_dir) in dir_list.iter().enumerate() {
                        let sub_name = sub_dir.file_name().unwrap().to_string_lossy().to_string();
                        if sub_name == file_name || sub_name > file_name {
                            state_index = index;
                            break;
                        }
                    }
                    path_states.insert(
                        cur_path.clone(),
                        PathPostorderIteratorPathState {
                            dir_list,
                            iter_dir_pos: state_index,
                            file_list,
                        },
                    );
                }
            }
        }

        Self {
            root_path,
            cur_path,
            path_states,
        }
    }

    pub fn get_state(&self) -> PathPostorderIteratorState {
        PathPostorderIteratorState {
            root_path: self.root_path.to_string_lossy().to_string(),
            cur_path: if self.cur_path.is_none() {
                None
            } else {
                Some(
                    self.cur_path
                        .as_ref()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                )
            },
        }
    }

    fn list_dir(path: &Path) -> BuckyResult<(Vec<PathBuf>, Vec<PathBuf>)> {
        let paths = std::fs::read_dir(path).map_err(|err| {
            let msg = format!(
                "read_dir {} err:{}",
                path.to_string_lossy().to_string(),
                err
            );
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let mut dir_list = Vec::new();
        let mut file_list = Vec::new();
        for sub in paths {
            match sub {
                Ok(sub) => {
                    let sub_path = path.join(sub.path());
                    if sub_path.is_file() {
                        file_list.push(sub.path());
                    } else {
                        dir_list.push(sub.path());
                    }
                }
                Err(err) => {
                    log::error!("read_dir err:{}", err);
                }
            }
        }

        dir_list.sort();

        Ok((dir_list, file_list))
    }

    fn get_item(&mut self, cur_path: PathBuf) -> BuckyResult<(PathBuf, Vec<PathBuf>)> {
        if let Ok((dir_list, file_list)) = Self::list_dir(cur_path.as_path()) {
            if dir_list.is_empty() {
                Ok((cur_path, file_list))
            } else {
                for (index, sub_dir) in dir_list.iter().enumerate() {
                    if let Ok((sub_path, sub_file_list)) = self.get_item(sub_dir.to_path_buf()) {
                        self.path_states.insert(
                            cur_path.clone(),
                            PathPostorderIteratorPathState {
                                dir_list,
                                iter_dir_pos: index,
                                file_list,
                            },
                        );
                        return Ok((sub_path, sub_file_list));
                    }
                }
                Err(BuckyError::new(BuckyErrorCode::Failed, ""))
            }
        } else {
            Ok((cur_path, Vec::new()))
        }
    }
}

impl Iterator for PathPostorderIterator {
    type Item = (PathBuf, Vec<PathBuf>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_path.is_none() {
            if let Ok((cur_path, file_list)) = self.get_item(self.root_path.clone()) {
                self.cur_path = Some(cur_path.clone());
                Some((cur_path, file_list))
            } else {
                None
            }
        } else {
            let mut cur_path = self.cur_path.take().unwrap();
            if cur_path == self.root_path {
                None
            } else {
                loop {
                    cur_path = cur_path.parent().unwrap().to_path_buf();

                    let (iter_dir_pos, dir_list) = match self.path_states.get(cur_path.as_path()) {
                        Some(state) => {
                            if state.dir_list.is_empty()
                                || state.iter_dir_pos >= state.dir_list.len() - 1
                            {
                                let state = self.path_states.remove(cur_path.as_path()).unwrap();
                                self.cur_path = Some(cur_path.clone());
                                return Some((cur_path, state.file_list));
                            } else {
                                (
                                    state.iter_dir_pos + 1,
                                    state.dir_list[state.iter_dir_pos + 1..]
                                        .iter()
                                        .map(|path| path.clone())
                                        .collect::<Vec<PathBuf>>(),
                                )
                            }
                        }
                        None => {
                            if cur_path == self.root_path {
                                return None;
                            } else {
                                continue;
                            }
                        }
                    };

                    let mut pos = 0;
                    while pos < dir_list.len() {
                        let sub_path = dir_list[pos].clone();
                        if let Ok((new_cur_path, file_list)) = self.get_item(sub_path) {
                            self.path_states
                                .get_mut(cur_path.as_path())
                                .unwrap()
                                .iter_dir_pos = iter_dir_pos + pos;
                            self.cur_path = Some(new_cur_path.clone());
                            return Some((new_cur_path, file_list));
                        }
                        pos += 1;
                    }

                    let state = self.path_states.remove(cur_path.as_path()).unwrap();
                    self.cur_path = Some(cur_path.clone());
                    return Some((cur_path, state.file_list));
                    //
                    // match self.iter_states.get(cur_path.as_path()) {
                    //     Some(state) => {
                    //         return if state.dir_list.is_empty() || state.iter_dir_pos >= state.dir_list.len() - 1 {
                    //             let state = self.iter_states.remove(cur_path.as_path()).unwrap();
                    //             self.cur_path = Some(cur_path);
                    //             Some(state.file_list)
                    //         } else {
                    //             let state = state.clone();
                    //             let mut iter_dir_pos = state.iter_dir_pos + 1;
                    //             while iter_dir_pos < state.dir_list.len() {
                    //                 let sub_path = state.dir_list[iter_dir_pos].clone();
                    //                 if let Ok((new_cur_path, file_list)) = self.get_item(sub_path) {
                    //                     self.iter_states.get_mut(cur_path.as_path()).unwrap().iter_dir_pos = iter_dir_pos;
                    //                     self.cur_path = Some(new_cur_path);
                    //                     return Some(file_list);
                    //                 }
                    //                 iter_dir_pos += 1;
                    //             }
                    //
                    //             let state = self.iter_states.remove(cur_path.as_path()).unwrap();
                    //             self.cur_path = Some(cur_path);
                    //             Some(state.file_list)
                    //         }
                    //     }
                    //     None => {
                    //         if cur_path == self.root_path {
                    //             return None;
                    //         }
                    //     }
                    // }
                }
            }
        }
    }
}

#[derive(RawEncode, RawDecode, Clone)]
pub enum BuildDirTaskStatus {
    Stopped,
    Running,
    Finished(ObjectId),
    Failed(BuckyError),
}

#[derive(RawEncode, RawDecode, Clone)]
pub(crate) struct DirTaskState {
    cur_path: Option<String>,
    cur_path_file_list: Vec<String>,
    it_state: Option<PathPostorderIteratorState>,
    sub_list: HashMap<String, HashMap<String, ObjectId>>,
}

struct TaskState {
    task_state: DirTaskState,
    task_status: BuildDirTaskStatus,
    canceler: Option<AbortHandle>,
    runnable_handle: Option<JoinHandle<()>>,
    building_task_list: HashSet<TaskId>,
}

impl TaskState {
    pub fn new() -> Self {
        Self {
            task_state: DirTaskState {
                cur_path: None,
                cur_path_file_list: vec![],
                it_state: None,
                sub_list: HashMap::new(),
            },
            task_status: BuildDirTaskStatus::Stopped,
            canceler: None,
            runnable_handle: None,
            building_task_list: HashSet::new(),
        }
    }

    pub fn new2(task_state: DirTaskState) -> Self {
        Self {
            task_state,
            task_status: BuildDirTaskStatus::Stopped,
            canceler: None,
            runnable_handle: None,
            building_task_list: HashSet::new(),
        }
    }
}
pub struct BuildDirTask {
    task_id: TaskId,
    task_store: Option<Arc<dyn TaskStore>>,
    local_path: String,
    owner: ObjectId,
    dec_id: ObjectId,
    chunk_size: u32,
    device_id: DeviceId,
    access: Option<u32>,
    noc: NamedObjectCacheRef,
    task_state: Arc<Mutex<TaskState>>,
    task_manager: Weak<TaskManager>,
    waiting_list: Arc<Mutex<Vec<AsyncCondvarRef>>>,
}

impl BuildDirTask {
    fn new(
        local_path: String,
        owner: ObjectId,
        dec_id: ObjectId,
        chunk_size: u32,
        access: Option<u32>,
        task_manager: Weak<TaskManager>,
        device_id: DeviceId,
        noc: NamedObjectCacheRef,
    ) -> Self {
        let mut sha2 = sha2::Sha256::new();
        sha2.input(local_path.as_bytes());
        sha2.input(owner.as_slice());
        sha2.input(dec_id.as_slice());
        sha2.input(chunk_size.to_be_bytes());
        sha2.input(BUILD_DIR_TASK.into().to_be_bytes());
        let task_id: TaskId = sha2.result().into();
        Self {
            task_id,
            task_store: None,
            local_path,
            owner,
            dec_id,
            chunk_size,
            access,
            device_id,
            noc,
            task_state: Arc::new(Mutex::new(TaskState::new())),
            task_manager,
            waiting_list: Arc::new(Mutex::new(vec![])),
        }
    }

    fn restore(
        local_path: String,
        owner: ObjectId,
        dec_id: ObjectId,
        chunk_size: u32,
        access: Option<u32>,
        task_manager: Weak<TaskManager>,
        device_id: DeviceId,
        noc: NamedObjectCacheRef,
        task_state: DirTaskState,
    ) -> Self {
        let mut sha2 = sha2::Sha256::new();
        sha2.input(local_path.as_bytes());
        sha2.input(owner.as_slice());
        sha2.input(dec_id.as_slice());
        sha2.input(chunk_size.to_be_bytes());
        sha2.input(BUILD_DIR_TASK.into().to_be_bytes());
        let task_id: TaskId = sha2.result().into();

        Self {
            task_id,
            task_store: None,
            local_path,
            owner,
            dec_id,
            chunk_size,
            access,
            device_id,
            noc,
            task_state: Arc::new(Mutex::new(TaskState::new2(task_state))),
            task_manager,
            waiting_list: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn build_dir(
        &self,
        path: PathBuf,
        sub_list: Vec<PathBuf>,
        start_pos: usize,
    ) -> BuckyResult<ObjectId> {
        let path_str = path.to_string_lossy().to_string();
        log::info!("build_dir {}", path_str.as_str());
        {
            let mut task_state = self.task_state.lock().unwrap();
            if !task_state
                .task_state
                .sub_list
                .contains_key(path_str.as_str())
            {
                task_state
                    .task_state
                    .sub_list
                    .insert(path_str.clone(), HashMap::new());
            }
        }
        let cpu_nums = num_cpus::get();

        let mut task_list = Vec::new();
        for sub in sub_list[start_pos..].iter() {
            let sub_file = path.join(sub);
            log::info!(
                "build_dir sub_file {}",
                sub_file.to_string_lossy().to_string()
            );
            let file_name = sub.file_name().unwrap().to_string_lossy().to_string();
            let task_manager = match self.task_manager.upgrade() {
                Some(task_manager) => task_manager,
                None => {
                    return Err(BuckyError::new(
                        BuckyErrorCode::Failed,
                        "task manager invalid",
                    ));
                }
            };

            let build_file_params = BuildFileParams {
                local_path: sub_file.to_string_lossy().to_string(),
                owner: self.owner.clone(),
                dec_id: self.dec_id.clone(),
                chunk_size: self.chunk_size,
                access: self.access.clone(),
            };

            let task_id = task_manager
                .create_task(
                    self.dec_id.clone(),
                    self.device_id.clone(),
                    BUILD_FILE_TASK,
                    build_file_params,
                )
                .await?;

            {
                let mut task_state = self.task_state.lock().unwrap();
                task_state.building_task_list.insert(task_id);
            }
            let task: JoinHandle<BuckyResult<(TaskId, String, ObjectId)>> =
                async_std::task::spawn(async move {
                    let ret: BuckyResult<(TaskId, String, ObjectId)> = async move {
                        task_manager.start_task(&task_id).await?;
                        task_manager.check_and_waiting_stop(&task_id).await;
                        let status = BuildFileTaskStatus::clone_from_slice(
                            task_manager
                                .get_task_detail_status(&task_id)
                                .await?
                                .as_slice(),
                        )?;
                        if let BuildFileTaskStatus::Finished(file) = status {
                            let file_id = file.desc().calculate_id();
                            Ok((task_id, file_name, file_id))
                        } else {
                            let msg = format!("build_file_object unexpect status");
                            log::error!("{}", msg.as_str());
                            Err(BuckyError::new(BuckyErrorCode::InvalidInput, msg))
                        }
                    }
                    .await;
                    ret
                });
            task_list.push(task);
            if task_list.len() >= cpu_nums {
                let ret = futures::future::select_all(task_list).await;
                let task_ret = ret.0;
                if let Ok((task_id, file_name, object_id)) = task_ret {
                    let mut task_state = self.task_state.lock().unwrap();
                    task_state
                        .task_state
                        .sub_list
                        .get_mut(path_str.as_str())
                        .unwrap()
                        .insert(file_name, object_id);
                    task_state.building_task_list.remove(&task_id);
                }
                task_list = ret.2;
            }

            let state_data = {
                let task_state = self.task_state.lock().unwrap();
                task_state.task_state.to_vec()?
            };

            if self.task_store.is_some() {
                self.task_store
                    .as_ref()
                    .unwrap()
                    .save_task_data(&self.task_id, state_data)
                    .await?;
            }
        }

        if task_list.len() > 0 {
            let rets = futures::future::join_all(task_list).await;
            for ret in rets {
                if let Ok((task_id, file_name, object_id)) = ret {
                    let mut task_state = self.task_state.lock().unwrap();
                    task_state
                        .task_state
                        .sub_list
                        .get_mut(path_str.as_str())
                        .unwrap()
                        .insert(file_name, object_id);
                    task_state.building_task_list.remove(&task_id);
                }
            }

            let state_data = {
                let task_state = self.task_state.lock().unwrap();
                task_state.task_state.to_vec()?
            };
            if self.task_store.is_some() {
                self.task_store
                    .as_ref()
                    .unwrap()
                    .save_task_data(&self.task_id, state_data)
                    .await?;
            }
        }
        let sub_list = {
            let mut task_state = self.task_state.lock().unwrap();
            task_state.task_state.cur_path = None;
            task_state.task_state.cur_path_file_list = Vec::new();
            task_state
                .task_state
                .sub_list
                .remove(path_str.as_str())
                .unwrap()
        };
        let noc = ObjectMapNOCCacheAdapter::new_noc_cache(self.noc.clone());
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(Some(self.dec_id.clone()), noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        let mut object_map = ObjectMap::new(
            ObjectMapSimpleContentType::Map,
            Some(self.owner.clone()),
            None,
        )
        .no_create_time()
        .build();

        for (sub_name, object_id) in sub_list.iter() {
            log::info!(
                "build dir {} {} {}",
                path_str.as_str(),
                sub_name.as_str(),
                object_id.to_string()
            );
            object_map
                .insert_with_key(&cache, sub_name, object_id)
                .await?;
        }

        let map_id = object_map.object_id();
        cache.put_object_map(
            &map_id,
            object_map,
            self.access.map(|v| AccessString::new(v)),
        )?;
        cache.commit().await?;

        Ok(map_id)
    }

    async fn run(&self) -> BuckyResult<ObjectId> {
        let (path, sub_list) = {
            let task_state = self.task_state.lock().unwrap();
            if task_state.task_state.cur_path.is_some() {
                let map = task_state
                    .task_state
                    .sub_list
                    .get(task_state.task_state.cur_path.as_ref().unwrap());
                if map.is_some() {
                    let map = map.unwrap();
                    let sub_list = task_state
                        .task_state
                        .cur_path_file_list
                        .iter()
                        .filter(|file_name| !map.contains_key(*file_name))
                        .map(|file_name| PathBuf::from(file_name))
                        .collect();
                    (
                        Some(PathBuf::from(
                            task_state.task_state.cur_path.as_ref().unwrap(),
                        )),
                        sub_list,
                    )
                } else {
                    (
                        Some(PathBuf::from(
                            task_state.task_state.cur_path.as_ref().unwrap(),
                        )),
                        Vec::new(),
                    )
                }
            } else {
                (None, Vec::new())
            }
        };
        if path.is_some() {
            let path = path.unwrap();
            let map_id = self.build_dir(path.clone(), sub_list, 0).await?;

            let state_data = {
                let mut task_state = self.task_state.lock().unwrap();
                task_state.task_state.cur_path = None;
                task_state.task_state.cur_path_file_list.clear();
                task_state.task_state.to_vec()?
            };

            if self.task_store.is_some() {
                self.task_store
                    .as_ref()
                    .unwrap()
                    .save_task_data(&self.task_id, state_data)
                    .await?;
            }

            if path.to_string_lossy().to_string() == self.local_path {
                return Ok(map_id);
            } else {
                let parent_str = path.parent().unwrap().to_string_lossy().to_string();
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                let mut task_state = self.task_state.lock().unwrap();
                if !task_state
                    .task_state
                    .sub_list
                    .contains_key(parent_str.as_str())
                {
                    let mut map = HashMap::new();
                    map.insert(file_name, map_id);
                    task_state.task_state.sub_list.insert(parent_str, map);
                }
            }
        }
        let mut it = {
            let mut task_state = self.task_state.lock().unwrap();
            if task_state.task_state.it_state.is_some() {
                PathPostorderIterator::from_state(task_state.task_state.it_state.take().unwrap())
            } else {
                PathPostorderIterator::new(PathBuf::from(self.local_path.clone()))
            }
        };
        let mut it_ret = it.next();
        while it_ret.is_some() {
            let (path, sub_list) = it_ret.unwrap();
            let it_state = it.get_state();
            let path_str = path.to_string_lossy().to_string();
            {
                let mut task_state = self.task_state.lock().unwrap();
                task_state.task_state.cur_path = Some(path_str.clone());
                task_state.task_state.cur_path_file_list = sub_list
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                task_state.task_state.it_state = Some(it_state);
            }

            let map_id = self.build_dir(path.clone(), sub_list, 0).await?;

            let state_data = {
                let mut task_state = self.task_state.lock().unwrap();
                task_state.task_state.cur_path = None;
                task_state.task_state.cur_path_file_list.clear();
                task_state.task_state.to_vec()?
            };

            if self.task_store.is_some() {
                self.task_store
                    .as_ref()
                    .unwrap()
                    .save_task_data(&self.task_id, state_data)
                    .await?;
            }

            if path.to_string_lossy().to_string() == self.local_path {
                return Ok(map_id);
            } else {
                let parent_str = path.parent().unwrap().to_string_lossy().to_string();
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                let mut task_state = self.task_state.lock().unwrap();
                log::info!(
                    "insert dir id {} {} {}",
                    parent_str.as_str(),
                    file_name.as_str(),
                    map_id.to_string()
                );
                if !task_state
                    .task_state
                    .sub_list
                    .contains_key(parent_str.as_str())
                {
                    let mut map = HashMap::new();
                    map.insert(file_name, map_id);
                    task_state.task_state.sub_list.insert(parent_str, map);
                } else {
                    task_state
                        .task_state
                        .sub_list
                        .get_mut(parent_str.as_str())
                        .unwrap()
                        .insert(file_name, map_id);
                }
            }
            it_ret = it.next();
        }
        Err(BuckyError::new(BuckyErrorCode::InvalidInput, ""))
    }
}

#[async_trait::async_trait]
impl Task for BuildDirTask {
    fn get_task_id(&self) -> TaskId {
        self.task_id.clone()
    }

    fn get_task_type(&self) -> TaskType {
        BUILD_DIR_TASK
    }

    fn get_task_category(&self) -> TaskCategory {
        BUILD_FILE_TASK_CATEGORY
    }

    fn need_persist(&self) -> bool {
        false
    }

    async fn get_task_status(&self) -> TaskStatus {
        let task_state = self.task_state.lock().unwrap();
        match task_state.task_status {
            BuildDirTaskStatus::Stopped => TaskStatus::Stopped,
            BuildDirTaskStatus::Running => TaskStatus::Running,
            BuildDirTaskStatus::Finished(_) => TaskStatus::Finished,
            BuildDirTaskStatus::Failed(_) => TaskStatus::Failed,
        }
    }

    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>) {
        self.task_store = Some(task_store);
    }

    async fn start_task(&self) -> BuckyResult<()> {
        unsafe {
            let this: &'static Self = std::mem::transmute(self);
            let (ft, handle) = futures::future::abortable(async move { this.run().await });

            {
                let mut state = self.task_state.lock().unwrap();
                state.canceler = Some(handle);
            }

            let task_id = self.task_id.clone();
            let task_state = self.task_state.clone();
            let task_store = self.task_store.clone();
            let runnable_handle = async_std::task::spawn(async move {
                let _: BuckyResult<()> = async move {
                    {
                        let mut tmp_data = task_state.lock().unwrap();
                        tmp_data.task_status = BuildDirTaskStatus::Running;
                    }
                    if task_store.is_some() {
                        task_store
                            .as_ref()
                            .unwrap()
                            .save_task_status(&task_id, TaskStatus::Running)
                            .await?;
                    }
                    match ft.await {
                        Ok(ret) => match ret {
                            Ok(object_id) => {
                                {
                                    let mut tmp_data = task_state.lock().unwrap();
                                    tmp_data.task_status = BuildDirTaskStatus::Finished(object_id);
                                    tmp_data.canceler = None;
                                    tmp_data.runnable_handle = None;
                                }
                                if task_store.is_some() {
                                    task_store
                                        .as_ref()
                                        .unwrap()
                                        .save_task_status(&task_id, TaskStatus::Finished)
                                        .await?;
                                }
                            }
                            Err(err) => {
                                {
                                    let mut tmp_data = task_state.lock().unwrap();
                                    tmp_data.task_status = BuildDirTaskStatus::Failed(err);
                                    tmp_data.canceler = None;
                                    tmp_data.runnable_handle = None;
                                }
                                if task_store.is_some() {
                                    task_store
                                        .as_ref()
                                        .unwrap()
                                        .save_task_status(&task_id, TaskStatus::Failed)
                                        .await?;
                                }
                            }
                        },
                        Err(_) => {
                            {
                                let mut tmp_data = task_state.lock().unwrap();
                                tmp_data.task_status = BuildDirTaskStatus::Stopped;
                            }
                            if task_store.is_some() {
                                task_store
                                    .as_ref()
                                    .unwrap()
                                    .save_task_status(&task_id, TaskStatus::Stopped)
                                    .await?;
                            }
                        }
                    }
                    Ok(())
                }
                .await;
            });
            {
                let mut task_state = self.task_state.lock().unwrap();
                task_state.runnable_handle = Some(runnable_handle);
            }
            Ok(())
        }
    }

    async fn pause_task(&self) -> BuckyResult<()> {
        self.stop_task().await
    }

    async fn stop_task(&self) -> BuckyResult<()> {
        let (canceler, runnable_handle) = {
            let mut data = self.task_state.lock().unwrap();
            (data.canceler.take(), data.runnable_handle.take())
        };
        if canceler.is_some() {
            canceler.unwrap().abort();
            if runnable_handle.is_some() {
                runnable_handle.unwrap().await;
            }
            if let Some(task_manager) = self.task_manager.upgrade() {
                let building_task_list = {
                    let task_state = self.task_state.lock().unwrap();
                    task_state.building_task_list.clone()
                };
                for task_id in building_task_list.iter() {
                    task_manager.stop_task(task_id).await?;
                }
                {
                    let mut task_state = self.task_state.lock().unwrap();
                    task_state.building_task_list.clear();
                }
            }

            Ok(())
        } else {
            let err = format!("task [{}] is not running!", self.task_id);
            log::error!("{}", err);
            Err(BuckyError::from((BuckyErrorCode::ErrorState, err)))
        }
    }

    async fn check_and_waiting_stop(&self) {
        let (runnable_handle, waiting) = {
            let mut waiting_list = self.waiting_list.lock().unwrap();
            let mut data = self.task_state.lock().unwrap();
            let handle = data.runnable_handle.take();
            if handle.is_some() {
                (handle, None)
            } else {
                if let BuildDirTaskStatus::Running = data.task_status {
                    let waiting = AsyncCondvar::new();
                    waiting_list.push(waiting.clone());
                    (None, Some(waiting))
                } else {
                    return;
                }
            }
        };

        if runnable_handle.is_some() {
            runnable_handle.unwrap().await;
            let mut waiting_list = self.waiting_list.lock().unwrap();
            for waiting in waiting_list.iter() {
                waiting.notify();
            }
            waiting_list.clear();
        } else {
            waiting.unwrap().wait().await;
        }
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        let task_state = self.task_state.lock().unwrap();
        task_state.task_status.to_vec()
    }
}

#[cfg(test)]
mod test_dir {
    use crate::util_api::local::PathPostorderIterator;
    use std::path::PathBuf;

    #[test]
    fn test_path_iter() {
        let it = PathPostorderIterator::new(PathBuf::from("/mnt/f/work/test"));
        let mut sum = 0;
        for (path, file_list) in it {
            println!("path {}", path.to_string_lossy().to_string());
            for file in file_list.iter() {
                println!("{}", path.join(file).to_string_lossy().to_string());
                sum += 1;
            }
        }

        println!("sum = {}", sum);
    }

    #[test]
    fn test_path_iter_resume() {
        let mut it = PathPostorderIterator::new(PathBuf::from("/mnt/f/work/test_poll"));
        let mut state = it.get_state();
        let mut sum = 0;
        for (path, file_list) in &mut it {
            println!("path {}", path.to_string_lossy().to_string());
            for file in file_list.iter() {
                println!("{}", path.join(file).to_string_lossy().to_string());
                sum += 1;
            }

            if sum > 200 {
                state = it.get_state();
                break;
            }
        }

        let it = PathPostorderIterator::from_state(state);
        for (path, file_list) in it {
            println!("path {}", path.to_string_lossy().to_string());
            for file in file_list.iter() {
                println!("{}", path.join(file).to_string_lossy().to_string());
                sum += 1;
            }
        }

        println!("sum = {}", sum);
    }
}
