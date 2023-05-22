use crate::*;
use cyfs_base::*;

use cyfs_bdt::{NdnTaskControlState, NdnTaskState};
use cyfs_core::TransContext;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::PathBuf;
use std::str::FromStr;

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
    Finished(u32/*upload_speed*/),
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
    pub context: Option<String>,
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

    // get TransContext object by object id
    pub context_id: Option<ObjectId>,

    // or get TransContext object by context_path excatly
    pub context_path: Option<String>,
}

pub struct TransGetContextOutputResponse {
    pub context: TransContext,
}

pub struct TransPutContextOutputRequest {
    pub common: NDNOutputRequestCommon,
    
    pub context: TransContext,
    pub access: Option<AccessString>,
}

#[derive(Debug)]
pub struct TransCreateTaskOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub object_id: ObjectId,
    // 保存到的本地目录or文件
    pub local_path: PathBuf,
    pub device_list: Vec<DeviceId>,

    pub group: Option<String>,
    pub context: Option<String>,

    // 任务创建完成之后自动启动任务
    pub auto_start: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransCreateTaskOutputResponse {
    pub task_id: String,
}

pub struct TransTaskOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub task_id: String,
}

// 控制传输一个任务的状态
#[derive(Debug)]
pub struct TransControlTaskOutputRequest {
    // 用以处理acl
    pub common: NDNOutputRequestCommon,
    pub task_id: String,
    pub action: TransTaskControlAction,
}

// get task state
#[derive(Debug)]
pub struct TransGetTaskStateOutputRequest {
    // 用以处理acl
    pub common: NDNOutputRequestCommon,
    pub task_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransGetTaskStateOutputResponse {
    pub state: TransTaskState,
    pub group: Option<String>,
}

// query tasks
#[derive(Debug)]
pub struct TransQueryTasksOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub task_status: Option<TransTaskStatus>,
    pub range: Option<(u64, u32)>,
}

pub struct TransQueryTasksOutputResponse {
    pub task_list: Vec<TransTaskInfo>,
}

// publish file
#[derive(Debug)]
pub struct TransPublishFileOutputRequest {
    // Same of NDN, now used for ACL process mainly
    pub common: NDNOutputRequestCommon,

    // The owner of the file or dir
    pub owner: ObjectId,

    // The local file full path of the file or dir
    pub local_path: PathBuf,

    // The chunk size of file and dir split, must be an integer multiple of 64
    pub chunk_size: u32,

    pub chunk_method: TransPublishChunkMethod, 

    pub access: Option<AccessString>,
    
    // The object_id of the file object to be published, if set, the file object is no longer calc internally and will direct load from NOC
    pub file_id: Option<ObjectId>,

    // The related objects
    pub dirs: Option<Vec<FileDirRef>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransPublishFileOutputResponse {
    pub file_id: ObjectId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[derive(serde_with::SerializeDisplay, serde_with::DeserializeFromStr)]
pub enum TransTaskGroupType {
    Download, 
    Upload
}

impl TransTaskGroupType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Download => "download", 
            Self::Upload => "upload",
        }
    }
}

impl Display for TransTaskGroupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TransTaskGroupType {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "download" => Self::Download,
            "upload" => Self::Upload,
            v @ _ => {
                let msg = format!("unknown trans group type: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        };

        Ok(ret)
    }
}

// get task group state
#[derive(Debug, Serialize, Deserialize)]
pub struct TransGetTaskGroupStateOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub group_type: TransTaskGroupType, 
    pub group: String,
    pub speed_when: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransGetTaskGroupStateOutputResponse {
    pub state: NdnTaskState,
    pub control_state: NdnTaskControlState,
    pub speed: Option<u32>,
    pub cur_speed: u32,
    pub history_speed: u32, 
    pub transfered: u64
}

// control task group
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransTaskGroupControlAction {
    Resume,
    Cancel,
    Pause,
    Close, 
    CloseRecursively
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransControlTaskGroupOutputRequest {
    pub common: NDNOutputRequestCommon,
    pub group_type: TransTaskGroupType, 
    pub group: String,
    pub action: TransTaskGroupControlAction,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransControlTaskGroupOutputResponse {
    pub control_state: NdnTaskControlState,
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_codec() {
        let gt = TransTaskGroupType::Download;
        let s = serde_json::to_string(&gt).unwrap();
        println!("{}", s);
        let s = s.trim_end_matches('"').trim_start_matches('"'); 
        assert_eq!(s, gt.as_str());

        let rs = TransTaskGroupType::from_str(&s).unwrap();
        assert_eq!(rs, gt);

        let req = TransGetTaskGroupStateOutputRequest {
            common: NDNOutputRequestCommon {
                req_path: None,
                dec_id: None,
                level: NDNAPILevel::Router,
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            group_type: TransTaskGroupType::Download,
            group: "test".to_owned(),
            speed_when: None,
        };

        let s = serde_json::to_string(&req).unwrap();
        println!("{}", s);
    }
}