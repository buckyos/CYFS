use cyfs_base::*;
use super::{
    resource::ResourceManager
};

// 对scheduler的接口
#[derive(Debug)]
pub enum TaskState {
    Pending, 
    Running(u32/*健康度*/),
    Paused,
    Canceled(BuckyErrorCode/*被cancel的原因*/), 
    Finished
}

pub trait TaskSchedule {
    fn schedule_state(&self) -> TaskState;
    fn resource(&self) -> &ResourceManager;
    fn start(&self) -> TaskState;
}

#[derive(Clone, Debug)]
pub enum TaskControlState {
    Downloading(u32/*速度*/, u32 /*进度*/), 
    Paused, 
    Canceled, 
    Finished(u32), 
    Err(BuckyErrorCode),
}

// 对应用的接口
pub trait DownloadTaskControl: Sync + Send {
    fn control_state(&self) -> TaskControlState;
    fn pause(&self) -> BuckyResult<TaskControlState>;
    fn resume(&self) -> BuckyResult<TaskControlState>;
    fn cancel(&self) -> BuckyResult<TaskControlState>;
}


pub trait DownloadTask: Send + Sync + std::fmt::Display + DownloadTaskControl + TaskSchedule {
    fn clone_as_download_task(&self) -> Box<dyn DownloadTask>;
}


