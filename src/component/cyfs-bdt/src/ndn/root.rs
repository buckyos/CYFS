use std::{
    sync::RwLock, 
    collections::LinkedList
};
use async_std::{
    sync::Arc, 
};
use cyfs_base::*;
use super::{
    scheduler::* 
};

// 实现里面不再分层了；所有的ChunkUploader从chunk manager便利就可以；
// 不用再加聚合到RootUploadTask里面了
// TODO: 这里面应当有个子 scheduler实现；在chunk粒度调度所有上传任务
pub struct RootUploadTask {
    resource: ResourceManager, 
}


impl RootUploadTask {
    fn new(owner: ResourceManager) -> Self {
        Self {
            resource: ResourceManager::new(Some(owner))
        }
    }
}

impl TaskSchedule for RootUploadTask {
    fn start(&self) -> TaskState {
        //do nothing
        TaskState::Running(0)
    }

    fn schedule_state(&self) -> TaskState {
        TaskState::Running(0)
    }

    fn resource(&self) -> &ResourceManager {
        &self.resource
    }
}


impl Scheduler for RootUploadTask {
    fn collect_resource_usage(&self) {
        //TODO
    }

    fn schedule_resource(&self) {
        //TODO
    }

    fn apply_scheduled_resource(&self) {
        //TODO
    }
}



pub struct RootDownloadTask {
    resource: ResourceManager, 
    tasks: RwLock<LinkedList<Box<dyn DownloadTask>>>
}


impl RootDownloadTask {
    fn new(owner: ResourceManager) -> Self {
        Self {
            resource: ResourceManager::new(Some(owner)), 
            tasks: RwLock::new(LinkedList::new())
        }
    }

    pub fn add_task(&self, task: Box<dyn DownloadTask>) -> BuckyResult<()> {
        self.tasks.write().unwrap().push_front(task.clone_as_download_task());
        let _ = task.start();
        Ok(())
    }
}

impl std::fmt::Display for RootDownloadTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RootDownloadTask")
    }
}

impl TaskSchedule for RootDownloadTask {
    fn start(&self) -> TaskState {
        // do nothing
        TaskState::Running(0)
    }

    fn schedule_state(&self) -> TaskState {
        TaskState::Running(0)
    }

    fn resource(&self) -> &ResourceManager {
        &self.resource
    }
}

impl Scheduler for RootDownloadTask {
    fn collect_resource_usage(&self) {
        // 移除掉已经完成的任务
        {
            let mut tasks = self.tasks.write().unwrap();
            let mut remain = LinkedList::new();
            loop {
                if let Some(task) = tasks.pop_front() {
                    let state = task.schedule_state(); 
                    if match state {
                        TaskState::Finished => true, 
                        TaskState::Canceled(_) => true, 
                        _ => false 
                    } {
                        let _ = self.resource().remove_child(task.resource());
                        task.resource().aggregate();
                        info!("{} remove task {} for finished/canceled", self, task);
                    } else {
                        remain.push_back(task);
                    }
                } else {
                    break;
                }
            }
            *tasks = remain;
        }
        self.resource().aggregate();
    }

    fn schedule_resource(&self) {
        //TODO
    }

    fn apply_scheduled_resource(&self) {
        //TODO
    }
}

struct RootTaskImpl {
    resource: ResourceManager, 
    download: RootDownloadTask, 
    upload: RootUploadTask
}

#[derive(Clone)]
pub struct RootTask(Arc<RootTaskImpl>);

impl RootTask {
    pub fn new(owner: ResourceManager) -> Self {
        let resource = ResourceManager::new(Some(owner));
        Self(Arc::new(RootTaskImpl {
            resource: resource.clone(), 
            download: RootDownloadTask::new(resource.clone()), 
            upload: RootUploadTask::new(resource.clone())
        }))
    }

    pub fn upload(&self) -> &RootUploadTask {
        &self.0.upload
    }

    pub fn download(&self) -> &RootDownloadTask {
        &self.0.download
    }

    pub fn resource(&self) -> &ResourceManager {
        &self.0.resource
    }

    pub(super) fn start(&self) {
        
    }
}


impl Scheduler for RootTask {
    fn collect_resource_usage(&self) {
        self.download().collect_resource_usage();
        self.upload().collect_resource_usage();
    }

    fn schedule_resource(&self) {
        //TODO： 根据device配置分配上传和下载资源占比
        self.download().schedule_resource();
        self.upload().schedule_resource();
    }

    fn apply_scheduled_resource(&self) {
        self.download().apply_scheduled_resource();
        self.upload().apply_scheduled_resource();
    }
}

