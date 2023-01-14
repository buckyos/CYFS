use super::output_request::*;
use crate::{NDNInputRequestCommon, TransTaskControlAction, TransTaskInfo, TransTaskStatus};
use cyfs_base::{BuckyResult, DeviceId, ObjectId, AccessString};
use cyfs_core::TransContext;
use cyfs_util::cache::FileDirRef;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub struct TransGetContextInputRequest {
    pub common: NDNInputRequestCommon,
    pub context_id: Option<ObjectId>,
    pub context_path: Option<String>,
}

pub type TransGetContextInputResponse = TransGetContextOutputResponse;

pub struct TransUpdateContextInputRequest {
    pub common: NDNInputRequestCommon,

    pub context: TransContext,
    pub access: Option<AccessString>,
}

#[derive(Debug)]
pub struct TransCreateTaskInputRequest {
    pub common: NDNInputRequestCommon,
    pub object_id: ObjectId,
    // 保存到的本地目录or文件
    pub local_path: PathBuf,
    pub device_list: Vec<DeviceId>,

    pub group: Option<String>,
    pub context: Option<String>,

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

pub type TransGetTaskStateInputResponse = TransGetTaskStateOutputResponse;

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

    pub access: Option<AccessString>,
    
    pub file_id: Option<ObjectId>,
    // 关联的dirs
    pub dirs: Option<Vec<FileDirRef>>,
}

#[derive(Debug)]
pub struct TransQueryTasksInputRequest {
    pub common: NDNInputRequestCommon,
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

// get task group state
#[derive(Debug)]
pub struct TransGetTaskGroupStateInputRequest {
    pub common: NDNInputRequestCommon,
    pub group_type: TransTaskGroupType, 
    pub group: String,
    pub speed_when: Option<u64>,
}

pub type TransGetTaskGroupStateInputResponse = TransGetTaskGroupStateOutputResponse;

// control task group
#[derive(Debug)]
pub struct TransControlTaskGroupInputRequest {
    pub common: NDNInputRequestCommon,
    pub group_type: TransTaskGroupType, 
    pub group: String,
    pub action: TransTaskGroupControlAction,
}

pub type TransControlTaskGroupInputResponse = TransControlTaskGroupOutputResponse;
