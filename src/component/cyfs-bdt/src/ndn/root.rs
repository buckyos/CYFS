use async_std::{
    sync::Arc, 
};
use crate::Timestamp;

use super::{
    scheduler::*,
    download::*,
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


struct RootTaskImpl {
    max_download_speed: u32, 
    download: DownloadGroup, 
    upload: RootUploadTask
}

#[derive(Clone)]
pub struct RootTask(Arc<RootTaskImpl>);

impl RootTask {
    pub fn new(max_download_speed: u32, history_speed: HistorySpeedConfig) -> Self {
        Self(Arc::new(RootTaskImpl {
            max_download_speed, 
            download: DownloadGroup::new(history_speed, None), 
            upload: RootUploadTask::new(ResourceManager::new(None))
        }))
    }

    pub fn upload(&self) -> &RootUploadTask {
        &self.0.upload
    }

    pub fn download(&self) -> &DownloadGroup {
        &self.0.download
    }

    pub fn on_schedule(&self, now: Timestamp) {
        self.download().calc_speed(now);
        self.download().on_drain(self.0.max_download_speed);
    }
}


impl Scheduler for RootTask {
    fn collect_resource_usage(&self) {
        self.upload().collect_resource_usage();
    }

    fn schedule_resource(&self) {
        //TODO： 根据device配置分配上传和下载资源占比
        self.upload().schedule_resource();
    }

    fn apply_scheduled_resource(&self) {
        self.upload().apply_scheduled_resource();
    }
}

