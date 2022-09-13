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
