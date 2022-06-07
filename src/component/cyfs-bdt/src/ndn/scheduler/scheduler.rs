use cyfs_base::*;

pub trait Scheduler {
    //第一遍遍历自下向上收集资源占用
    //第二遍遍历自上向下分配资源
    //第三遍遍历自上向下根据分配的资源执行实际的操作
    fn collect_resource_usage(&self);
    fn schedule_resource(&self);
    fn apply_scheduled_resource(&self);
}

pub trait EventScheduler {
    fn upload_scheduler_event(&self, requester: &DeviceId) -> BuckyResult<()>;
    fn download_scheduler_event(&self, requester: &DeviceId) -> BuckyResult<()>;
}
