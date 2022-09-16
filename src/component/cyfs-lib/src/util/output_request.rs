use crate::{prelude::*, GlobalStateAccessMode};
use crate::zone::ZoneRole;
use cyfs_base::*;
use cyfs_core::ZoneId;
use cyfs_core::*;
use cyfs_bdt::SnStatus;
use std::convert::TryFrom;

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct UtilOutputRequestCommon {
    // 请求路径，可为空
    pub req_path: Option<String>,

    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl Default for UtilOutputRequestCommon {
    fn default() -> Self {
        Self {
            req_path: None,
            dec_id: None,
            target: None,
            flags: 0,
        }
    }
}

impl fmt::Display for UtilOutputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req_path: {:?}", self.req_path)?;

        if let Some(dec_id) = &self.dec_id {
            write!(f, ", dec_id: {}", dec_id)?;
        }

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target.to_string())?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetDeviceOutputRequest {
    pub common: UtilOutputRequestCommon,
}

impl Display for UtilGetDeviceOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)
    }
}

impl UtilGetDeviceOutputRequest {
    pub fn new() -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetDeviceOutputResponse {
    pub device_id: DeviceId,
    pub device: Device,
}

impl Display for UtilGetDeviceOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "device_id: {}", self.device_id)
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetZoneOutputRequest {
    pub common: UtilOutputRequestCommon,

    pub object_id: Option<ObjectId>,
    pub object_raw: Option<Vec<u8>>,
}

impl UtilGetZoneOutputRequest {
    pub fn new(object_id: Option<ObjectId>, object_raw: Option<Vec<u8>>) -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
            object_id,
            object_raw,
        }
    }
}

impl Display for UtilGetZoneOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        write!(f, "object_id: {:?}", self.object_id)
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetZoneOutputResponse {
    pub zone_id: ZoneId,
    pub zone: Zone,
    pub device_id: DeviceId,
}

impl Display for UtilGetZoneOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "zone_id: {}", self.zone_id)?;

        write!(f, "device_id: {:?}", self.device_id)
    }
}

#[derive(Debug, Clone)]
pub struct UtilResolveOODOutputRequest {
    pub common: UtilOutputRequestCommon,

    pub object_id: ObjectId,
    pub owner_id: Option<ObjectId>,
}

impl UtilResolveOODOutputRequest {
    pub fn new(object_id: ObjectId, owner_id: Option<ObjectId>) -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
            object_id,
            owner_id,
        }
    }
}

impl Display for UtilResolveOODOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        write!(f, "object_id: {:?}", self.object_id)?;

        if let Some(owner_id) = &self.owner_id {
            write!(f, "owner_id: {:?}", owner_id)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UtilResolveOODOutputResponse {
    pub device_list: Vec<DeviceId>,
}

impl Display for UtilResolveOODOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "device_list: {:?}", self.device_list)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum OODNetworkType {
    Unknown,
    Intranet,
    Extranet,
}

impl Display for OODNetworkType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let v = match self {
            Self::Unknown => "unknown",
            Self::Intranet => "intranet",
            Self::Extranet => "extranet",
        };

        write!(f, "{}", v)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OODStatus {
    pub network: OODNetworkType,

    pub first_ping: u64,
    pub first_success_ping: u64,
    pub last_success_ping: u64,

    pub last_ping: u64,
    pub last_ping_result: u16,

    pub ping_count: u32,
    pub ping_success_count: u64,

    // 当前连续失败的次数，成功后重置
    pub cont_fail_count: u64,

    pub ping_avg_during: u64,
    pub ping_max_during: u64,
    pub ping_min_during: u64,

    // current zone's ood
    pub ood_device_id: DeviceId,

    // is root-state sync enable on this device
    pub enable_sync: bool,

    // device local root-state
    pub device_root_state: ObjectId,
    pub device_root_state_revision: u64,

    // zone local root-state
    pub zone_root_state: Option<ObjectId>,
    pub zone_root_state_revision: u64,
}

#[derive(Debug, Clone)]
pub struct UtilGetOODStatusOutputRequest {
    pub common: UtilOutputRequestCommon,
}

impl Display for UtilGetOODStatusOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)
    }
}

impl UtilGetOODStatusOutputRequest {
    pub fn new() -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UtilGetOODStatusOutputResponse {
    pub status: OODStatus,
}

impl Display for UtilGetOODStatusOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "status: {:?}", self.status)
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetNOCInfoOutputRequest {
    pub common: UtilOutputRequestCommon,
}

impl Display for UtilGetNOCInfoOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)
    }
}

impl UtilGetNOCInfoOutputRequest {
    pub fn new() -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UtilGetNOCInfoOutputResponse {
    pub stat: NamedObjectCacheStat,
}

impl Display for UtilGetNOCInfoOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stat: {:?}", self.stat)
    }
}

// 设备的一些静态信息
#[derive(Debug, Clone)]
pub struct DeviceStaticInfo {
    // 当前设备id
    pub device_id: DeviceId,
    pub device: Device,

    // 当前设备是不是ood
    pub is_ood_device: bool,

    pub ood_work_mode: OODWorkMode,

    pub zone_role: ZoneRole,

    pub root_state_access_mode: GlobalStateAccessMode,
    pub local_cache_access_mode: GlobalStateAccessMode,
    
    // 当前zone的主ood id
    pub ood_device_id: DeviceId,

    // 当前所属zone
    pub zone_id: ZoneId,

    // 当前zone的owner
    pub owner_id: Option<ObjectId>,

    // 当前协议栈的cyfs根目录
    pub cyfs_root: String,
}

#[derive(Debug, Clone)]
pub struct UtilGetDeviceStaticInfoOutputRequest {
    pub common: UtilOutputRequestCommon,
}

impl Display for UtilGetDeviceStaticInfoOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)
    }
}

impl UtilGetDeviceStaticInfoOutputRequest {
    pub fn new() -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetDeviceStaticInfoOutputResponse {
    pub info: DeviceStaticInfo,
}

impl Display for UtilGetDeviceStaticInfoOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "info: {:?}", self.info)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum BdtNetworkAccessType {
    NAT,
    WAN,
}

impl fmt::Display for BdtNetworkAccessType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let v = match self {
            Self::NAT => "nat",
            Self::WAN => "wan",
        };

        write!(f, "{}", v)
    }
}

impl FromStr for BdtNetworkAccessType {
    type Err = BuckyError;

    fn from_str(s: &str) -> BuckyResult<Self> {
        match s {
            "nat" => Ok(Self::NAT),
            "wan" => Ok(Self::WAN),
            _ => {
                let msg = format!("unknown BdtNetworkAccessType value: {}", s);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidData, msg))
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct BdtNetworkAccessEndpoint {
    pub lan_ep: Endpoint,
    pub wan_ep: Endpoint,

    pub access_type: BdtNetworkAccessType,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct BdtNetworkAccessSn {
    pub sn: DeviceId,
    pub sn_status: SnStatus,
}

#[derive(Debug, Clone)]
pub struct BdtNetworkAccessInfo {
    pub v4: Vec<BdtNetworkAccessEndpoint>,
    pub v6: Vec<BdtNetworkAccessEndpoint>,

    pub sn: Vec<BdtNetworkAccessSn>,
}

impl Default for BdtNetworkAccessInfo {
    fn default() -> Self {
        Self {
            v4: Vec::new(),
            v6: Vec::new(),
            sn: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetNetworkAccessInfoOutputRequest {
    pub common: UtilOutputRequestCommon,
}

impl Display for UtilGetNetworkAccessInfoOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)
    }
}

impl UtilGetNetworkAccessInfoOutputRequest {
    pub fn new() -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetNetworkAccessInfoOutputResponse {
    pub info: BdtNetworkAccessInfo,
}

impl Display for UtilGetNetworkAccessInfoOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "info: {:?}", self.info)
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetSystemInfoOutputRequest {
    pub common: UtilOutputRequestCommon,
}

impl Display for UtilGetSystemInfoOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)
    }
}

impl UtilGetSystemInfoOutputRequest {
    pub fn new() -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilGetSystemInfoOutputResponse {
    pub info: cyfs_util::SystemInfo,
}

impl Display for UtilGetSystemInfoOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "info: {:?}", self.info)
    }
}

#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub version: String,
    pub channel: CyfsChannel,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct UtilGetVersionInfoOutputRequest {
    pub common: UtilOutputRequestCommon,
}

impl Display for UtilGetVersionInfoOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)
    }
}

impl UtilGetVersionInfoOutputRequest {
    pub fn new() -> Self {
        Self {
            common: UtilOutputRequestCommon::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UtilGetVersionInfoOutputResponse {
    pub info: VersionInfo,
}

impl Display for UtilGetVersionInfoOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "info: {:?}", self.info)
    }
}

#[derive(Debug, Clone)]
pub struct UtilBuildFileOutputRequest {
    pub common: UtilOutputRequestCommon,
    pub local_path: PathBuf,
    pub owner: ObjectId,
    pub chunk_size: u32,
}

pub struct UtilBuildFileOutputResponse {
    pub object_id: ObjectId,
    pub object_raw: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum BuildDirType {
    Zip,
}

impl TryFrom<u16> for BuildDirType {
    type Error = BuckyError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(BuildDirType::Zip),
            v @ _ => {
                let msg = format!("unknown build dir type value {}", v);
                log::error!("{}", msg.as_str());
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct UtilBuildDirFromObjectMapOutputRequest {
    pub common: UtilOutputRequestCommon,
    pub object_map_id: ObjectId,
    pub dir_type: BuildDirType,
}

pub struct UtilBuildDirFromObjectMapOutputResponse {
    pub object_id: ObjectId,
}
