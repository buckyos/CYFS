use crate::{NDNInputRequestCommon, TransTaskControlAction, TransTaskInfo, TransTaskStatus};
use cyfs_base::{DeviceId, ObjectId, BuckyResult};
use cyfs_core::TransContext;
use std::path::PathBuf;
use cyfs_util::cache::FileDirRef;
use serde::{
    Deserialize,
    Serialize,
};

pub struct TransGetContextInputRequest {
    pub common: NDNInputRequestCommon,
    pub context_name: String,
}

pub struct TransUpdateContextInputRequest {
    pub common: NDNInputRequestCommon,
    pub context: TransContext,
}

#[derive(Debug)]
pub struct TransCreateTaskInputRequest {
    pub common: NDNInputRequestCommon,
    pub object_id: ObjectId,
    // 保存到的本地目录or文件
    pub local_path: PathBuf,
    pub device_list: Vec<DeviceId>,
    pub context_id: Option<ObjectId>,
    pub auto_start: bool,
}

impl TransCreateTaskInputRequest {
    pub fn check_valid(&self) -> BuckyResult<()> {
        self.common.check_param_with_referer(&self.object_id)
    }
}

// 控制传输一个任务的状态
#[derive(Debug)]
pub struct TransControlTaskInputRequest {
    // 用以处理acl
    pub common: NDNInputRequestCommon,
    pub task_id: String,
    pub action: TransTaskControlAction,
}

#[derive(Debug)]
pub struct TransGetTaskStateInputRequest {
    // 用以处理acl
    pub common: NDNInputRequestCommon,
    pub task_id: String,
}

#[derive(Debug)]
pub struct TransPublishFileInputRequest {
    // 用以处理acl
    pub common: NDNInputRequestCommon,
    // 文件所属者
    pub owner: ObjectId,

    // 文件的本地路径
    pub local_path: PathBuf,
    // chunk大小
    pub chunk_size: u32,

    pub file_id: Option<ObjectId>,
    // 关联的dirs
    pub dirs: Option<Vec<FileDirRef>>,
}

#[derive(Debug)]
pub struct TransQueryTasksInputRequest {
    pub common: NDNInputRequestCommon,
    pub context_id: Option<ObjectId>,
    pub task_status: Option<TransTaskStatus>,
    pub range: Option<(u64, u32)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransPublishFileInputResponse {
    pub file_id: ObjectId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransCreateTaskInputResponse {
    pub task_id: String,
}

pub struct TransQueryTasksInputResponse {
    pub task_list: Vec<TransTaskInfo>,
}
