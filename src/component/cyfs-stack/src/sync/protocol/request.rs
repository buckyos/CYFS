use cyfs_base::*;
use cyfs_lib::*;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DeviceSyncState {
    Online,
    OnlineAccept,
    Offline,
}

impl fmt::Display for DeviceSyncState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match &*self {
            Self::Online => "online",
            Self::OnlineAccept => "online_accept",
            Self::Offline => "offline",
        };

        fmt::Display::fmt(s, f)
    }
}

impl FromStr for DeviceSyncState {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s {
            "online" => Self::Online,
            "online_accept" => Self::OnlineAccept,
            "offline" => Self::Offline,
            v @ _ => {
                let msg = format!("unknown DeviceSyncState: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Debug)]
pub(crate) struct SyncPingRequest {
    pub device_id: DeviceId,
    pub zone_role: ZoneRole,

    pub root_state: ObjectId,
    pub root_state_revision: u64,

    pub state: DeviceSyncState,

    // local owner's body update time
    pub owner_update_time: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct SyncPingResponse {
    pub zone_root_state: ObjectId,
    pub zone_root_state_revision: u64,
    pub zone_role: ZoneRole,
    pub ood_work_mode: OODWorkMode,
    pub owner: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub(crate) struct SyncDiffRequest {
    pub category: GlobalStateCategory,
    pub path: String,
    pub dec_id: Option<ObjectId>,
    pub current: Option<ObjectId>,
}

#[derive(Debug)]
pub(crate) struct SyncDiffResponse {
    pub revision: u64,

    // if target not exists, will return none
    pub target: Option<ObjectId>,
    pub objects: Vec<SelectResponseObjectInfo>,
}

#[derive(Debug)]
pub(crate) struct SyncObjectsRequest {
    pub begin_seq: u64,
    pub end_seq: u64,
    pub list: Vec<ObjectId>,
}

// 目前使用SelectResponse来作为同步结果返回，结构是一致的
pub(crate) type SyncObjectsResponse = SelectResponse;

pub(crate) type SyncZoneRequest = SyncPingResponse;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SyncChunksRequest {
    pub chunk_list: Vec<ChunkId>,
    pub states: Vec<ChunkState>,
}

impl JsonCodecAutoWithSerde for SyncChunksRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SyncChunksResponse {
    pub result: Vec<bool>,
}

impl JsonCodecAutoWithSerde for SyncChunksResponse {}
