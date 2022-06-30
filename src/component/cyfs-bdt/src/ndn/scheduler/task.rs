use cyfs_base::*;
use super::{
    resource::ResourceManager
};
use super::statistic::*;

// 对scheduler的接口
#[derive(Debug)]
pub enum TaskState {
    Pending, 
    Running(u32/*健康度*/),
    Paused,
    Canceled(BuckyErrorCode/*被cancel的原因*/), 
    Redirect(DeviceId),
    Finished
}

pub trait TaskSchedule {
    fn schedule_state(&self) -> TaskState;
    fn resource(&self) -> &ResourceManager;
    fn statistic_task(&self) -> Option<DynamicStatisticTask> {
        None
    }
    fn start(&self) -> TaskState;
}

#[derive(Clone)]
pub enum TaskControlState {
    Downloading(usize/*速度*/, usize /*进度*/), 
    Paused, 
    Canceled, 
    Finished, 
    Redirect(DeviceId),
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


