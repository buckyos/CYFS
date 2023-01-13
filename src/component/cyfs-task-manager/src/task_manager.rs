use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use cyfs_base::*;
use crate::{Locker, Task, TaskCategory, TaskFactory, TaskId, TaskManagerStore, TaskStatus, TaskStore, TaskType};

struct TaskInfo {
    pub task: Arc<Box<dyn Task>>,
    pub complete_time: Option<u64>,
    pub dec_list: Vec<DecInfo>,
}

impl TaskInfo {
    pub fn new(task: Arc<Box<dyn Task>>, dec_list: Vec<DecInfo>) -> Self {
        Self {
            task,
            complete_time: None,
            dec_list
        }
    }
}

#[derive(Debug, Clone, RawEncode, RawDecode)]
pub struct DecInfoV1 {
    dec_id: ObjectId,
    source: DeviceId,
}

#[derive(Debug, Clone, RawEncode, RawDecode)]
pub enum DecInfo {
    V1(DecInfoV1)
}

impl DecInfo {
    pub fn new(dec_id: ObjectId, source: DeviceId) -> Self {
        Self::V1(DecInfoV1 { dec_id, source })
    }

    pub fn dec_id(&self) -> &ObjectId {
        match self {
            Self::V1(info) => &info.dec_id
        }
    }

    pub fn source(&self) -> &DeviceId {
        match self {
            Self::V1(info) => &info.source
        }
    }
}

pub struct TaskManager {
    task_factory_map: Mutex<HashMap<TaskType, Arc<dyn TaskFactory>>>,
    task_manager_store: Arc<dyn TaskManagerStore>,
    task_store: Arc<dyn TaskStore>,
    task_map: async_std::sync::Mutex<HashMap<TaskId, TaskInfo>>,
}

impl TaskManager {
    pub async fn new(task_manager_store: Arc<dyn TaskManagerStore>, task_store: Arc<dyn TaskStore>) -> BuckyResult<Arc<Self>> {
        let task_manager = Arc::new(Self {
            task_factory_map: Mutex::new(Default::default()),
            task_store,
            task_manager_store,
            task_map: async_std::sync::Mutex::new(Default::default())
        });

        // task_manager.task_manager_store.clear_can_delete_task().await?;
        let tmp_task_manager = Arc::downgrade(&task_manager);
        async_std::task::spawn(async move {
            loop {
                match tmp_task_manager.upgrade() {
                    Some(task_manager) => {
                        if let Err(e) = task_manager.clear_task().await {
                            log::error!("task manager clear task err {}", e);
                        }
                        async_std::task::sleep(Duration::from_secs(600)).await;
                    },
                    None => {
                        break;
                    }
                }
            }
        });
        Ok(task_manager)
    }

    async fn clear_task(&self) -> BuckyResult<()> {
        let mut task_map = self.task_map.lock().await;
        let mut clear_task = Vec::new();
        for (task_id, task_info) in task_map.iter_mut() {
            let task_status = task_info.task.get_task_status().await;
            if task_status == TaskStatus::Stopped || task_status == TaskStatus::Finished || task_status == TaskStatus::Failed {
                if task_info.complete_time.is_none() {
                    task_info.complete_time = Some(bucky_time_now());
                } else {
                    if bucky_time_now() - task_info.complete_time.unwrap() > 600000000 {
                        clear_task.push(task_id.clone());
                    }
                }
            } else {
                task_info.complete_time = None;
            }
        }

        for clear_id in clear_task.iter() {
            task_map.remove(clear_id);
        }

        Ok(())
    }

    pub fn register_task_factory(&self, factory: impl TaskFactory) -> BuckyResult<()> {
        let mut task_factory_map = self.task_factory_map.lock().unwrap();
        let task_type = factory.get_task_type();
        let ret = task_factory_map.insert(factory.get_task_type(), Arc::new(factory));
        if ret.is_none() {
            Ok(())
        } else {
            Err(BuckyError::new(BuckyErrorCode::AlreadyExists, format!("task factory {} has exist", task_type.into())))
        }
    }

    fn get_task_factory(&self, task_type: &TaskType) -> Option<Arc<dyn TaskFactory>> {
        let task_factory_map = self.task_factory_map.lock().unwrap();
        match task_factory_map.get(task_type) {
            Some(factory) => Some(factory.clone()),
            None => None
        }
    }

    pub async fn resume_task(&self) -> BuckyResult<()> {
        let task_data_list = self.task_manager_store.get_tasks_by_status(TaskStatus::Running).await?;
        for (task_id, task_type, params, data) in task_data_list {
            match self.get_task_factory(&task_type) {
                Some(factory) => {
                    let dec_list = self.task_manager_store.get_dec_list(&task_id).await?;
                    let mut task = match factory.restore(TaskStatus::Stopped, params.as_slice(), data.as_slice()).await {
                        Ok(task) => task,
                        Err(e) => {
                            let msg = format!("restore task {} failed.{}", task_id.to_string(), e);
                            log::error!("{}", msg.as_str());
                            continue;
                        }
                    };
                    task.set_task_store(self.task_store.clone()).await;
                    task.start_task().await?;
                    let mut task_map = self.task_map.lock().await;
                    task_map.insert(task_id.clone(), TaskInfo::new(Arc::new(task), dec_list));
                },
                None => {
                    continue;
                }
            }
        }

        Ok(())
    }

    pub async fn create_task<P: RawEncode>(&self, dec_id: ObjectId, source: DeviceId, task_type: TaskType, task_param: P) -> BuckyResult<TaskId> {
        log::info!("create_task dec_id {} task_type {}", dec_id.to_string(), task_type.into());
        match self.get_task_factory(&task_type) {
            Some(factory) => {
                let param = task_param.to_vec()?;
                let mut task = factory.create(param.as_slice()).await?;
                if task.need_persist() {
                    task.set_task_store(self.task_store.clone()).await;
                }
                let task_id = task.get_task_id();
                let _locker = Locker::get_locker(format!("task_manager_{}", task_id.to_string())).await;
                {
                    let ret = {
                        let mut task_map = self.task_map.lock().await;
                        if task_map.contains_key(&task_id) {
                            let task = task_map.get_mut(&task_id).unwrap();
                            if Self::add_dec(&mut task.dec_list, dec_id, source.clone()) && task.task.need_persist() {
                                self.task_manager_store.add_dec_info(&task_id,
                                                                     task.task.get_task_category(),
                                                                     task.task.get_task_status().await, task.dec_list.last().unwrap()).await?;
                            }
                            Some(task.task.get_task_id())
                        } else {
                            None
                        }
                    };
                    if let Some(task_id) = ret {
                        return Ok(task_id);
                    }
                }

                if task.need_persist() {
                    match self.task_manager_store.get_task(&task_id).await {
                        Ok((task_category, _task_type, task_status, task_param, task_data)) => {
                            let mut task = factory.restore(task_status, task_param.as_slice(), task_data.as_slice()).await?;
                            task.set_task_store(self.task_store.clone()).await;

                            let dec_list = self.task_manager_store.get_dec_list(&task_id).await?;
                            let mut task_map = self.task_map.lock().await;
                            let mut task_info = TaskInfo::new(Arc::new(task), dec_list);
                            if Self::add_dec(&mut task_info.dec_list, dec_id, source) {
                                self.task_manager_store.add_dec_info(&task_id, task_category, task_info.task.get_task_status().await, task_info.dec_list.last().unwrap()).await?;
                            }
                            task_map.insert(task_id.clone(), task_info);

                            Ok(task_id)
                        },
                        Err(e) => {
                            if e.code() == BuckyErrorCode::NotFound {
                                let dec_list = vec![DecInfo::new(dec_id, source)];
                                let task_info = TaskInfo::new(Arc::new(task), dec_list.clone());
                                self.task_manager_store.add_task(&task_id,
                                                                 task_info.task.get_task_category(),
                                                                 task_info.task.get_task_type(),
                                                                 task_info.task.get_task_status().await,
                                                                 dec_list,
                                                                 param).await?;
                                let mut task_map = self.task_map.lock().await;
                                task_map.insert(task_id.clone(), task_info);
                                Ok(task_id)
                            } else {
                                Err(e)
                            }
                        }
                    }
                } else {
                    let dec_list = vec![DecInfo::new(dec_id, source)];
                    let task_info = TaskInfo::new(Arc::new(task), dec_list.clone());
                    let mut task_map = self.task_map.lock().await;
                    task_map.insert(task_id.clone(), task_info);
                    Ok(task_id)
                }
            },
            None => {
                let msg = format!("not support task type {}", task_type);
                log::error!("{}", msg.as_str());
                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub async fn start_task(&self, task_id: &TaskId) -> BuckyResult<()> {
        log::info!("start_task {}", task_id.to_string());
        let _locker = Locker::get_locker(format!("task_manager_{}", task_id.to_string())).await;
        let task = {
            let task_map = self.task_map.lock().await;
            match task_map.get(task_id) {
                Some(task_info) => Some(task_info.task.clone()),
                None => None,
            }
        };
        match task {
            Some(task) => {
                task.start_task().await
            }
            None => {
                let (_task_category, task_type, task_status, task_param, task_data) = self.task_manager_store.get_task(task_id).await?;
                match self.get_task_factory(&task_type) {
                    Some(factory) => {
                        let dec_list = self.task_manager_store.get_dec_list(task_id).await?;
                        let mut task = factory.restore(task_status, task_param.as_slice(), task_data.as_slice()).await?;
                        task.set_task_store(self.task_store.clone()).await;
                        let task = {
                            let mut task_map = self.task_map.lock().await;
                            if !task_map.contains_key(task_id) {
                                task_map.insert(task_id.clone(), TaskInfo::new(Arc::new(task), dec_list));
                            }
                            task_map.get(task_id).unwrap().task.clone()
                        };
                        task.start_task().await
                    }
                    None => {
                        let msg = format!("not find task id {}", task_id.to_string());
                        log::error!("{}", msg.as_str());
                        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                    }
                }
            }
        }
    }

    pub async fn check_and_waiting_stop(&self, task_id: &TaskId) {
        log::info!("check_and_waiting_stop {}", task_id.to_string());
        let task = {
            let task_map = self.task_map.lock().await;
            match task_map.get(task_id) {
                Some(task_info) => Some(task_info.task.clone()),
                None => None,
            }
        };
        match task {
            Some(task) => {
                task.check_and_waiting_stop().await;
            },
            None => {}
        }
    }

    pub async fn get_task_detail_status(&self, task_id: &TaskId) -> BuckyResult<Vec<u8>> {
        log::debug!("will get_task_detail_status {}", task_id);
        let task = {
            let task_map = self.task_map.lock().await;
            match task_map.get(task_id) {
                Some(task_info) => Some(task_info.task.clone()),
                None => None,
            }
        };
        match task {
            Some(task) => {
                task.get_task_detail_status().await
            },
            None => {
                let (_task_category, task_type, task_status, task_param, task_data) = self.task_manager_store.get_task(task_id).await?;
                match self.get_task_factory(&task_type) {
                    Some(factory) => {
                        let dec_list = self.task_manager_store.get_dec_list(task_id).await?;
                        let mut task = factory.restore(task_status, task_param.as_slice(), task_data.as_slice()).await?;
                        task.set_task_store(self.task_store.clone()).await;
                        let task = {
                            let mut task_map = self.task_map.lock().await;
                            if !task_map.contains_key(task_id) {
                                task_map.insert(task_id.clone(), TaskInfo::new(Arc::new(task), dec_list));
                            }
                            task_map.get(task_id).unwrap().task.clone()
                        };
                        task.get_task_detail_status().await
                    }
                    None => {
                        let msg = format!("task not found! task={}", task_id);
                        log::error!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                    }
                }
            }
        }
    }

    pub async fn pause_task(&self, task_id: &TaskId) -> BuckyResult<()> {
        log::info!("will pause_task {}", task_id);
        let _locker = Locker::get_locker(format!("task_manager_{}", task_id.to_string())).await;
        let task = {
            let task_map = self.task_map.lock().await;
            match task_map.get(task_id) {
                Some(task_info) => task_info.task.clone(),
                None => {
                    return Ok(());
                }
            }
        };
        task.pause_task().await
    }

    pub async fn stop_task(&self, task_id: &TaskId) -> BuckyResult<()> {
        log::info!("will stop_task {}", task_id);
        let _locker = Locker::get_locker(format!("task_manager_{}", task_id)).await;
        let task = {
            let mut task_map = self.task_map.lock().await;
            task_map.remove(task_id)
        };
        match task {
            Some(task) => {
                task.task.stop_task().await
            },
            None => {
                warn!("stop task but not found! task={}", task_id);
                Ok(())
            }
        }
    }

    pub async fn remove_task(&self, dec_id: &ObjectId, source: &DeviceId, task_id: &TaskId) -> BuckyResult<()> {
        log::info!("remove_task dec_id {} task_id {}", dec_id.to_string(), task_id.to_string());
        let _locker = Locker::get_locker(format!("task_manager_{}", task_id.to_string())).await;

        let mut task_map = self.task_map.lock().await;
        match task_map.get_mut(task_id) {
            None => {
                let mut dec_list = self.task_manager_store.get_dec_list(&task_id).await?;
                if Self::remove_dec(&mut dec_list, dec_id, source) {
                    self.task_manager_store.delete_dec_info(task_id, dec_id, source).await?;
                }

                if dec_list.len() == 0 {
                    self.task_manager_store.delete_task(task_id).await?;
                    task_map.remove(task_id);
                }
            }
            Some(info) => {
                if Self::remove_dec(&mut info.dec_list, dec_id, source) {
                    if info.task.need_persist() {
                        self.task_manager_store.delete_dec_info(task_id, dec_id, source).await?;
                    }
                }
                if info.dec_list.len() == 0 {
                    if info.task.need_persist() {
                        self.task_manager_store.delete_task(task_id).await?;
                    }
                    task_map.remove(task_id);
                }
            }
        }

        Ok(())
    }

    pub async fn remove_task_by_task_id(&self, task_id: &TaskId) -> BuckyResult<()> {
        log::info!("remove_task task_id {}", task_id.to_string());
        let _locker = Locker::get_locker(format!("task_manager_{}", task_id.to_string())).await;

        let mut task_map = self.task_map.lock().await;
        match task_map.get_mut(task_id) {
            None => {
                self.task_manager_store.delete_task(task_id).await?;
            }
            Some(task) => {
                if task.task.need_persist() {
                    self.task_manager_store.delete_task(task_id).await?;
                }
                task_map.remove(task_id);
            }
        }

        Ok(())
    }

    pub async fn get_tasks_by_task_id(&self, task_id_list: &[TaskId]) -> BuckyResult<Vec<(TaskId, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>> {
        self.task_manager_store.get_tasks_by_task_id(task_id_list).await
    }

    pub async fn get_tasks_by_category(&self, category: TaskCategory) -> BuckyResult<Vec<(TaskId, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>> {
        self.task_manager_store.get_tasks_by_category(category).await
    }

    fn add_dec(dec_list: &mut Vec<DecInfo>, new_dec: ObjectId, source: DeviceId) -> bool {
        let mut find = false;
        for dec in dec_list.iter_mut() {
            if dec.dec_id() == &new_dec && dec.source() == &source {
                find = true;
                break;
            }
        }

        if !find {
            dec_list.push(DecInfo::new(new_dec, source));
        }

        !find
    }

    fn exist_dec(dec_list: & Vec<DecInfo>, new_dec: &ObjectId, source: &DeviceId) -> bool {
        for dec in dec_list.iter() {
            if dec.dec_id() == new_dec && dec.source() == source {
                return true;
            }
        }
        false
    }

    fn remove_dec(dec_list: &mut Vec<DecInfo>, dest_dec: &ObjectId, source: &DeviceId) -> bool {
        let mut find = false;
        for (index, dec) in dec_list.iter().enumerate() {
            if dec.dec_id() == dest_dec && dec.source() ==  source {
                dec_list.remove(index);
                find = true;
                break;
            }
        }
        find
    }
}

pub mod test_task_manager {
    use std::sync::Arc;
    use cyfs_base::BuckyResult;
    use crate::{SQLiteTaskStore, TaskManager};

    pub async fn create_test_task_manager() -> BuckyResult<Arc<TaskManager>> {
        let store = Arc::new(SQLiteTaskStore::new(":memory:").await?);
        store.init().await?;
        let task_manager = TaskManager::new(store.clone(), store).await?;
        Ok(task_manager)
    }
}
