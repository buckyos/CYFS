use cyfs_base::*;
use cyfs_lib::*;

use http_types::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Eq, PartialEq)]
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
impl fmt::Debug for DeviceSyncState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
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

#[derive(Clone)]
pub struct SyncResponseObjectMetaInfo {
    pub insert_time: u64,
    pub create_dec_id: Option<ObjectId>,
    pub context: Option<String>,
    pub last_access_rpath: Option<String>,
    pub access_string: Option<u32>,
}

impl fmt::Display for SyncResponseObjectMetaInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "insert_time:{}", self.insert_time)?;

        if let Some(v) = &self.create_dec_id {
            write!(f, ", create_dec_id:{} ", v)?;
        }
        if let Some(v) = &self.context {
            write!(f, ", context:{} ", v)?;
        }
        if let Some(v) = &self.last_access_rpath {
            write!(f, ", last_access_rpath:{} ", v)?;
        }
        write!(f, ", access:{:?}", self.access_string)?;

        Ok(())
    }
}
impl fmt::Debug for SyncResponseObjectMetaInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Debug, Clone, RawEncode, RawDecode)]
pub struct SyncResponseObjectInfo {
    pub meta: SyncResponseObjectMetaInfo,
    pub object: Option<NONObjectInfo>,
}

impl SyncResponseObjectInfo {
    fn from_meta(meta: SyncResponseObjectMetaInfo) -> Self {
        Self { meta, object: None }
    }

    fn meta(&self) -> &SyncResponseObjectMetaInfo {
        &self.meta
    }
}

impl fmt::Display for SyncResponseObjectInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ", self.meta)?;

        if let Some(obj) = &self.object {
            write!(f, "object:{} ", obj)?;
        }

        Ok(())
    }
}

pub struct SyncObjectsResponse {
    pub objects: Vec<SelectResponseObjectInfo>,
}

impl fmt::Display for SyncObjectsResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "size:{}", self.objects.len())?;
        for item in &self.objects {
            write!(f, ",{}", item)?;
        }

        Ok(())
    }
}
impl fmt::Debug for SyncObjectsResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl SyncObjectsResponse {
    pub fn encode_objects(
        http_resp: &mut Response,
        objects: Vec<SelectResponseObjectInfo>,
    ) -> BuckyResult<()> {
        if objects.is_empty() {
            return Ok(());
        }

        let buf = objects.to_vec()?;

        debug!(
            "will send sync objects all_buf: len={}, count={}",
            buf.len(),
            objects.len(),
            //hex::encode(&all_buf)
        );

        http_resp.set_body(buf);

        Ok(())
    }

    pub fn into_resonse(self) -> BuckyResult<Response> {
        let mut resp = RequestorHelper::new_response(StatusCode::Ok);
        if !self.objects.is_empty() {
            Self::encode_objects(&mut resp, self.objects)?;
        }

        Ok(resp)
    }

    pub async fn from_respone(mut resp: Response) -> BuckyResult<Self> {
        let all_buf = resp.body_bytes().await.map_err(|e| {
            let msg = format!("read select resp body bytes error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let objects = if all_buf.len() > 0 {
            let (objects, _) = Vec::raw_decode(&all_buf)?;
            objects
        } else {
            vec![]
        };

        debug!(
            "recv sync objects all_buf: len={}, count={}",
            all_buf.len(),
            objects.len(),
        );

        Ok(Self { objects })
    }
}

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
