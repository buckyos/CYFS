use crate::*;
use cyfs_base::*;

use serde::{
    Deserialize,
    Serialize,
};
use std::path::PathBuf;
use std::str::FromStr;
use cyfs_core::TransContext;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransTaskOnAirState {
    pub download_percent: u32,
    pub download_speed: u32,
    pub upload_speed: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransTaskState {
    Pending,
    Downloading(TransTaskOnAirState),
    Paused,
    Canceled,
    Finished(u32 /*upload_speed*/),
    Err(BuckyErrorCode),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransTaskStatus {
    Stopped,
    Running,
    Finished,
    Failed,
}

pub struct TransTaskInfo {
    pub task_id: String,
    pub context_id: Option<ObjectId>,
    pub object_id: ObjectId,
    pub local_path: PathBuf,
    pub device_list: Vec<DeviceId>,
}
/*
#[serde(deserialize_with = "error_code_deserialize")]
#[serde(serialize_with = "error_code_serialize")]
fn error_code_serialize<S>(err: &BuckyErrorCode, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u16(err.as_u16())
}

fn error_code_deserialize<'de, D>(d: D) -> Result<BuckyErrorCode, D::Error>
where
    D: Deserializer<'de>,
{
    struct BuckyErrorCodeVisitor {}
    impl<'de> Visitor<'de> for BuckyErrorCodeVisitor {
        type Value = BuckyErrorCode;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("BuckyErrorCode")
        }

        fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
        where E: de::Error, {
            Ok(BuckyErrorCode::from(v))
        }
    }

    d.deserialize_u16(BuckyErrorCodeVisitor {})
}
*/

#[derive(Clone, Debug)]
pub enum TransTaskControlAction {
    Start,
    Stop,
    Delete,
}

impl ToString for TransTaskControlAction {
    fn to_string(&self) -> String {
        (match *self {
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Delete => "Delete",
        })
        .to_owned()
    }
}

impl FromStr for TransTaskControlAction {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "Start" => Self::Start,
            "Stop" => Self::Stop,
            "Delete" => Self::Delete,
            v @ _ => {
                let msg = format!("unknown TransTaskControlAction: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

pub struct TransGetContextOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub context_name: String,
}

pub struct TransPutContextOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub context: TransContext,
}

#[derive(Debug)]
pub struct TransCreateTaskOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub object_id: ObjectId,
    // ????????????????????????or??????
    pub local_path: PathBuf,
    pub device_list: Vec<DeviceId>,
    pub context_id: Option<ObjectId>,
    // ??????????????????????????????????????????
    pub auto_start: bool,
}

pub struct TransTaskOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub task_id: String,
}

// ?????????????????????????????????
#[derive(Debug)]
pub struct TransControlTaskOutputRequest {
    // ????????????acl
    pub common: NDNOutputRequestCommon,
    pub task_id: String,
    pub action: TransTaskControlAction,
}


#[derive(Debug)]
pub struct TransGetTaskStateOutputRequest {
    // ????????????acl
    pub common: NDNOutputRequestCommon,
    pub task_id: String,
}

#[derive(Debug)]
pub struct TransQueryTasksOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub context_id: Option<ObjectId>,
    pub task_status: Option<TransTaskStatus>,
    pub range: Option<(u64, u32)>,
}

#[derive(Debug)]
pub struct TransPublishFileOutputRequest {
    // ????????????acl
    pub common: NDNOutputRequestCommon,
    // ???????????????
    pub owner: ObjectId,

    // ?????????????????????
    pub local_path: PathBuf,
    // chunk??????
    pub chunk_size: u32,

    // ???????????????????????????ID????????????????????????????????????????????????
    pub file_id: Option<ObjectId>,

    // ?????????dirs
    pub dirs: Option<Vec<FileDirRef>>,
}

#[derive(Debug)]
pub struct TransPublishFileOutputResponse {
    pub file_id: ObjectId,
}

pub struct TransCreateTaskOutputResponse {
    pub task_id: String,
}

pub struct TransQueryTasksOutputResponse {
    pub task_list: Vec<TransTaskInfo>,
}
